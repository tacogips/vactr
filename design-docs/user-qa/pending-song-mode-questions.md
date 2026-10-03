# Song Mode Production Integration: Pending Decisions

These choices come from the production integration contract in
`design-docs/specs/design-song-mode.md#production-integration-contract-route-authority-to-playback-2026-10-03`
(2026-10-03, session 249). No user has decided them yet. Until the user answers,
the recommendation for each one applies by default.

## SM1: Index retention work under production default limits

- **Question**: The native render path uses `src/cli/render.rs`
  `preparation_limits()` with `SongLimits::default()`. Its `max_nodes` is
  16,384. A measured chord/two-use fixture spent 15,388 of 16,384 units on
  retention alone. Pre-Reserve retention on the issued path may therefore
  refuse the requirement-level end-to-end song under production defaults. What
  should happen in that case?
- **Options**:
  - (a) Keep `SongLimits::default()` unchanged. Raise only the song work
    allowance in the host/CLI preparation limits, within the 1,000,000 that
    `SongLimits::validate` already permits. Document the measured need.
  - (b) Raise `SongLimits::default().max_nodes`.
  - (c) Keep the defaults and accept truthful refusal of larger songs.
- **Recommendation**: (a). This work allowance limits CPU work, not resource
  pools or musical admission. Changing the public default affects every legacy
  caller. Measure first. Change limits only if the end-to-end song is actually
  refused, and record the measured work in the wave-3 receipt.

## SM2: Runtime first-time execution of unseen seeds

- **Question**: Must this batch support executing seeds that are first seen at
  playback time, after Reserve?
- **Options**:
  - (a) No. A static Song has a finite set of placements. Pre-Reserve retention
    executes every `:vary` and `:same` placement once under the original
    ledger, and additive replay seeding keeps those executions. A missing
    record refuses without VM access.
  - (b) Yes. Add a sealed first-execution guard with an aggregate opening bound
    for runtime execution.
- **Recommendation**: (a). Static songs cannot produce an unseen seed after
  preparation. Option (b) only matters for live or dynamic seed sources, which
  are outside this release (design: Timing and randomness). Evidence for (a) is
  that `:vary` repeats differ musically and `:same` repeats are identical on
  the production path.

## SM3: Unowned rustfmt rewrites in the working tree

Added 2026-10-03, session 250.

- **Question**: The session-250 issue says that
  `src/song/snapshot/resources.rs`, `src/song/snapshot/reservations_tests.rs`
  and `src/sched/runtime/song/clock_tests.rs` were restored to `HEAD`. At
  session-250 intake, however, all three are modified and unstaged again. The
  changes look like a rustfmt rewrite of assert chains and struct literals
  (about 76 insertions and 80 deletions). No plan owns these paths. What should
  happen to them before the Route8 gates are rerun?
- **Options**:
  - (a) The orchestrator saves the diff to
    `tmp/song-mode-riela/session250-unowned-rewrite.patch` and restores exactly
    these three paths to `HEAD`. Every cohort audit then allows hash changes
    only on declared writePaths.
  - (b) Leave the files modified, and have the audits keep excusing them as
    "pre-existing drift".
  - (c) Commit the rewrite as a separate formatting-only commit outside this
    batch, then rebaseline the cohort.
- **Recommendation**: (a). It produces the state the operator described, and the
  saved patch makes it reversible. Option (b) restores the drift clause that the
  operator asked to remove. Option (c) changes the 944-input baseline, which no
  plan declares. Workers and reviewers never perform the restore.

## SM4: Preparation admission of dynamic Index operands

Added 2026-10-04, session 251.

- **Question**: Route preparation's density preflight
  (`src/song/routing/density/index.rs`, `DensityWalk::index_support`) refuses
  an Index operand whose support needs canonical realization, for example a
  late-bound function variable in the Index list (`[cut nil]`, which is
  `PParam::Late` and gives `kind=Late, issuance=None`). Session 250's resolver
  fixtures used this form and failed during preparation, before
  `resolve_issued_event` ran. An honest bound needs the retained canonical
  Index evidence held by `RouteAuthorityView`. The density walk cannot reach
  that evidence unless `src/song/routing/density.rs`,
  `src/song/routing/prepare/builder.rs` and `src/song/routing/prepare.rs` are
  edited. The last two are accepted SONG-ROUTE8 paths. Should this batch
  support dynamic Index operands at preparation?
- **Options**:
  - (a) No. Both ledgers keep refusing dynamic Index operands at preparation,
    and the refusal stays truthful. Resolver fixtures use statically admissible
    Index lists. Their callback evidence comes from the Slice subject lambda
    and reusable Part functions. Dynamic Index preparation admission stays
    future work, together with the `RequiresUniformBound` ceiling in the
    canonical occupancy plan.
  - (b) Yes. Authorize `density.rs`, `prepare/builder.rs` and `prepare.rs` as
    additional SONG-ISSUED-RESOLUTION writePaths. The issued ledger then passes
    the retained canonical Index support for each site into the density walk.
    The legacy ledger keeps refusing.
- **Recommendation**: (a). The operator authorized only `density/index.rs`.
  A density bound computed without retained evidence would be invented, which
  the existing `Uncertifiable(DynamicTiming)` refusal already forbids.
  The full objective (static Part code, an Index/Slice-sourced track, and
  NeedsJointGeometry resolved on the issued path) does not need dynamic Index
  values. Under (a), `density/index.rs` stays authorized but may stay
  unedited.
