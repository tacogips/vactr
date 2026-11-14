# Removed live bindings on Update implementation plan

**Status**: Completed
**Design Reference**: ../../design-docs/specs/design-editor-header.md#removed-live-bindings-on-whole-document-update
**Created**: 2026-10-24
**Last Updated**: 2026-10-24

## Deliverables

| Module | Deliverable | Status |
|---|---|---|
| Session | Stamp authoritative source-file ownership on accepted runtime live slots and reconcile absent sinks after successful full evaluation | Completed |
| Dependency graph | Retire obsolete bind ownership without deleting session names or enabling removed forms to replay | Completed |
| Regression tests | Eleven native cases, including asynchronous SetVar repair and failed-attempt retirement | Completed |
| Verification | Final native/WASM gates, fresh release, frontend and browser evidence | Completed |

## Contracts

- `Slot.binding_file: Option<FileId>`: source file of the last runtime-accepted binding; failed attempts and tempo/transport changes never transfer it. `Runtime::bind` stamps it after successful acceptance on every drain path, and revoke clears it.
- `Session::reconcile_live_bindings(&mut self, file: FileId, evaluated: &BTreeSet<FormId>)`: naturally revoke absent file-owned sinks after a fully successful submission.
- `Evaluator::retire_live_binding(&mut self, key: SlotKey, file: FileId)`: withdraw obsolete reactive sink ownership for that file.

No implementation bodies in this plan. Full-document reconciliation preserves the existing per-form transaction model; it does not make the entire document an atomic rollback transaction.

## Tasks and dependencies

1. Implement accepted-runtime ownership tracking and graph retirement.
2. Integrate successful full-document reconciliation; depends on task 1.
3. Add native regressions and run verification/build; depends on task 2.
4. Coordinating agent joins actual-WASM/browser evidence and archives the plan.

## Completion criteria

- [x] Commented/deleted/empty-document sinks naturally stop on successful whole Update.
- [x] Other slots/files retain playback, including after a failed attempted takeover.
- [x] Parse/check/eval/dry-run errors never trigger absence cleanup.
- [x] Partial eval retains incremental semantics.
- [x] Removed reactive forms never resurrect; pending announcements are retracted.
- [x] Native regressions and required Rust gates pass.
- [x] Fresh release WASM passes coordinating-agent frontend/browser verification.
- [x] Every touched Rust file stays below 1000 lines.

## Progress log

### Session: 2026-10-24 — diagnosis and design
Session eval is incremental and never revokes missing forms. Dependency graph sink ownership also survives deletion, allowing later variable writes to rebuild removed forms. Runtime dry-run failure can diverge attempted graph ownership from accepted audible ownership. The fix therefore tracks accepted slot generations/file ownership and withdraws obsolete graph sink ownership only after successful full-document evaluation. All existing dirty changes are preserved.

### Session: 2026-10-24 — implementation and first gates

Implemented per-document accepted live-slot ownership and successful-full-document reconciliation. `Slot.binding_revision: u64` changes only for accepted pattern/texture rebindings; tempo and transport generations cannot falsely transfer ownership after a rejected bind. Missing sinks use existing per-slot natural Revoke, cancel pending rebinds and previews, preserve other-file slots, and retire dependency-graph bind ownership while leaving complete write sets intact to prevent removed forms from replaying. Console-file evaluation and partial selections remain incremental. No-op reconciliation does not add a runtime drain.

The first eight focused tests pass; the ninth adds successful same-slot takeover coverage. The affected first gate passes 454 tests (2020 unrelated tests skipped), including session, namespace/dependency graph and scheduler suites. Native check, strict all-target Clippy, portable WASM check and fmt pass. Final nine-case regression pass and fresh release build are pending, followed by coordinating-agent actual-WASM/browser evidence. Production audit and coordinating-agent review are complete; no further source edits are planned. Every touched source remains below 1000 lines (evaluator 962; Session 924; other touched files smaller). Existing dirty changes are preserved.

### Session: 2026-10-24 — final native verification

The mandated check-and-test-after-modify agent reports focused 9/9 tests and affected 455/455 tests passing (2020 unrelated tests skipped). Affected command: `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run --lib --no-fail-fast -E 'test(session::) | test(ns::) | test(sched::)'`. Logs: `/tmp/vactr-remove-live-regressions-final2.log` and `/tmp/vactr-remove-live-focused-final2.log`.

Native `cargo check`, all-target `cargo clippy --all-targets -- -D warnings`, portable `cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm`, and `cargo fmt --check`, all through mise with `CARGO_TERM_QUIET=true`, exit 0. Logs: `/tmp/vactr-remove-live-check-final.log`, `/tmp/vactr-remove-live-clippy-final2.log`, `/tmp/vactr-remove-live-wasm-check.log`, `/tmp/vactr-remove-live-fmt-final2.log`. `git diff --check` passes. Fresh final-source release build is running; no production edits followed the audit or final gates. Browser verification and plan archival remain owned by the coordinating agent.

### Session: 2026-10-24 — asynchronous acceptance audit refinement

The coordinating-agent audit identified tick-time `SetVar`/`SetTweak` and `stage_now` drains outside `eval_doc`; the earlier session-only snapshot registry could miss accepted reactive takeovers. Superseded that approach with authoritative `Slot.binding_file` provenance stamped inside `Runtime::bind` only after successful binding acceptance, automatically covering every drain path. Removed the duplicate DocState registry, accepted-revision counter and per-evaluation generation snapshots. Revoke clears accepted provenance. Reconciliation now separately retires obsolete attempted dependency-graph sink owners even when they never owned an audible slot; this prevents failed-then-deleted source from reviving later while preserving another file's sound.

Added two regressions for tick-time SetVar repair transferring ownership, and deleting a failed attempt before repairing its variable. Refined suite has eleven cases. Earlier 455-test gates and release artifact are not accepted evidence for this refined source; verification and release will rerun after the final audit.

### Session: 2026-10-24 — authoritative ownership final gates

The final refined source passes 11/11 dedicated regression cases and 457/457 affected namespace, scheduler and session tests (2020 unrelated tests skipped). Same nextest selection as above: `test(session::) | test(ns::) | test(sched::)`. Final refined native check, strict all-target Clippy, portable host-wasm check and format check all exit 0, with no production edits after audit/review. Logs: `/tmp/vactr-remove-live-regressions-refined.log`, `/tmp/vactr-remove-live-focused-refined.log`, `/tmp/vactr-remove-live-check-refined.log`, `/tmp/vactr-remove-live-clippy-refined.log`, `/tmp/vactr-remove-live-wasm-check-refined.log`, `/tmp/vactr-remove-live-fmt-refined.log`. The mandatory verifier ran all gates; `git diff --check` passes and every touched source is below1000 lines.

Coordinating-agent review accepted the runtime acceptance stamp and separate graph retirement approach, with no further findings. The fresh final3 release build is running in `/tmp/vactr-remove-live-wasm-release-final3.log`; the earlier artifact is obsolete and must not be used as final evidence. No additional production modifications remain planned. Actual-WASM/full-frontend and browser verification remain for coordinating-agent evidence join and archival.

### Session: 2026-10-24 — final refined release artifact

`CARGO_TERM_QUIET=true mise run build-wasm-release` passed, exit0, log `/tmp/vactr-remove-live-wasm-release-final3.log`. Final accepted artifact: `target/wasm32-unknown-unknown/release/vactr.wasm`, 6,961,138 bytes, SHA-256 `9d2807bb512c6dacd2fd070d043e085ec4841f6c74bf71fb707b5eacfbda8b0a`. Built from the authoritative runtime-ownership source after the final457-test gates; the earlier111aff artifact is obsolete. The verifier checked artifact validity, retained name section, and absence of inspected DWARF sections. All build/test process handles are terminal. Plan stays active for coordinating-agent actual-WASM/full-frontend/browser evidence join and archival.

### Session: 2026-10-24 — actual-WASM and browser acceptance

Before the fix, the real-WASM regression in `editor/test/wasm/canvas-clock.test.ts` reproduced future d1 events after its source was commented out while d2/d3 continued (`/tmp/vactr-comment-update-before.log`). The final artifact passes the full frontend suite2422/2422 across167 files, including that regression's other-file, empty-document and final comment-only checks. Command: `VACTR_WASM=../target/wasm32-unknown-unknown/release/vactr.wasm mise exec -- npm --prefix editor run test`; exit0, `/tmp/vactr-comment-update-frontend-final.log`.

TypeScript checking and Vite production build pass (`/tmp/vactr-comment-update-build-final.log`); the bundled WASM SHA matches the accepted release. Existing tree-sitter build warnings are unchanged. `mise exec -- npm --prefix editor run e2e:header` exits0 in Chromium and WebKit at1440/768/390px (`/tmp/vactr-comment-update-browser-final.log`, `tmp/editor-header/results.json`). New UI coverage starts sound, comments out its code, clicks Update, waits for authoritative inactive telemetry, and verifies no new playing events over1.2s without pressing Stop. Visually inspected `tmp/editor-header/chromium-comment-update-stopped.png`: commented code, Stopped state, successful eval and disabled Update. Existing nil, edit-marker, pause, file-history and clipboard checks also pass. `git diff --check` passes; all requested work is complete.
