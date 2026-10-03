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
