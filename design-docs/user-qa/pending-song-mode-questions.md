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
