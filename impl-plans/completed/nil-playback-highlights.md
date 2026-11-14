# Nil playback highlights implementation plan

**Status**: Completed
**Design Reference**: ../../design-docs/specs/design-editor-header.md#rest-playback-highlights
**Created**: 2026-10-23
**Last Updated**: 2026-10-24

## Deliverables

| Module | Deliverable | Status |
|---|---|---|
| Query | Playback-only silent rest capture and timing-frame projection | Completed |
| Scheduler | Stage, announce and confirm silent markers without host output or hit signals | Completed |
| Protocol | Optional `rest` flag on playing telemetry | Completed |
| Verification | Rest timing/silence/lifecycle regression coverage | Completed |

## Public contracts

`pub(crate) fn query_playback(p: &Pat, span: TimeSpan, cx: &mut QueryCtx<'_>) -> QueryResult`

This scheduler-only entry appends silent `Value::Nil` markers after ordinary musical composition completes. Public `query` and traced finite-song realization retain their existing semantics. `PlayingEvent.rest: bool` and optional wire `WirePlaying.rest` distinguish markers from sound/hit telemetry.

## Tasks and dependencies

1. Capture provenance-bearing rest leaves in a separate playback query channel; preserve ordinary query semantics.
2. Feed markers through existing staging and revocation; depends on task 1.
3. Publish the optional rest flag, and verify timing and silence; depends on task 2.

## Completion criteria

- [x] Live nil steps publish highlights at their transformed scheduled intervals.
- [x] Musical events, sound output and hit/control signals retain existing behavior.
- [x] Rest markers obey existing scheduling, stop, mute and rebind lifecycle.
- [x] Focused regression tests, compilation and lint checks pass.
- [x] Fresh release WASM and actual browser visual checks joined by coordinating agent.
- [x] Every touched Rust source remains below 1000 lines.

## Progress log

### Session: 2026-10-23
Inspected nil step elimination and existing canonical timing frames, scheduler staging, bounded announcements and playing publication. Implementing a separate playback-only rest capture channel to avoid changing pattern composition semantics.

### Session: 2026-10-24 — implementation and production audit

Implemented playback-only nil capture, an independent 4096-marker bound, structural sampling relations, affine frame mapping, and exact transformed-end occurrence identity for reused sources at different speeds. Query samples used only for computation suppress capture. Probability/chunk filtering, striate ranking, failed subtrees and failed structural samples discard inactive markers. Musical output, print/fault results and query budget are isolated from visual-only predicate work. Silent committed/announced events reuse scheduler ledgers, revocation, epochs, provenance and bounded queues; they never reach audio/MIDI/OSC sinks or hit/control windows.

Scope is live scheduler bindings on native and browser tiers. Finite-song pre-realized sources are unchanged, as confirmed with the coordinating agent from the user's live-pipeline examples. Added ten scheduler regressions covering literal source spans, nil-only activity/silence, fast/rev/nesting, reused-source durations, gain sampling, euclid/segment sampling, probability endpoints, chunk selection, failed sibling/structural samples, mute retraction and Stop.

Earlier affected verification passed 468 tests with nine rest fixtures; final ten-fixture run and final source release build are pending. Initial deep reversal stack regression was fixed by keeping playback-only projection temporaries in a non-inlined helper; the existing deep recursion boundary now passes without changing stack limits. Actual visual browser verification and final evidence join belong to the coordinating agent. Keep this plan active until that join.

### Session: 2026-10-24 — final Rust verification

The final ten-rest-fixture source passes the complete affected Rust scope: 469/469 tests, with 1997 unrelated tests skipped. Exact command: `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run --lib --no-fail-fast -E 'test(pattern::) | test(sched::) | test(session::)'`. This includes canonical song-clock and unchanged deep-recursion boundary tests. Log: `/tmp/vactr-rest-regressions-final5.log`.

Native `cargo check`, all-target `cargo clippy --all-targets -- -D warnings`, portable `cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm`, and `cargo fmt --check`, each through mise with `CARGO_TERM_QUIET=true`, all exit 0. Logs: `/tmp/vactr-rest-check-final5.log`, `/tmp/vactr-rest-clippy-final5.log`, `/tmp/vactr-rest-wasm-check-final5.log`, `/tmp/vactr-rest-fmt-final5.log`. The mandated check-and-test-after-modify agent ran every final gate. `git diff --check` passes and every touched Rust source is below 1000 lines.

Final production audit covers provenance/revisions, affine and structural timing, computational sample isolation, probability/chunk selection, error rollback, overlapping query identities, reused-source duration identity, silent host/hit behavior, independent visual budget, and Stop/mute/retraction lifecycle. No source edits remain planned; release WASM build and coordinating-agent browser/full-frontend evidence join are pending.

### Session: 2026-10-24 — fresh release artifact

`CARGO_TERM_QUIET=true mise run build-wasm-release` passed, exit 0, log `/tmp/vactr-rest-wasm-release-final5.log`. Fresh artifact: `target/wasm32-unknown-unknown/release/vactr.wasm`, 6,954,335 bytes, SHA-256 `b3d782a6d9b45034e65ba20a9ac87863d56d227001f8bc20ef192f490c3259ea`. It was built after the final occurrence-identity production correction and final Rust gates; obsolete earlier builds were canceled and are not accepted evidence. All verification processes are terminal. Actual browser/full-frontend verification remains for the coordinating-agent join.

### Session: 2026-10-24 — final browser evidence join

Against that artifact, the full frontend suite passes2420/2420, including actual raw-WASM nil telemetry feeding the editor HighlightScheduler (`editor/test/wasm/canvas-clock.test.ts`). Chromium and WebKit header e2e checks pass, with nil source spans visibly active and stopped Update verified (`tmp/editor-header/results.json`, `tmp/editor-header/*-nil-active.png`; `/tmp/vactr-editor-header-final2.log`). The artifact validates, retains exactly one name section, and has no DWARF debug sections. All final source, runtime and visual evidence is joined; the plan is complete.
