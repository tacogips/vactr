# CE-FINAL: Canvas editor verification implementation plan

**planId**: CE-FINAL
**planPath**: impl-plans/active/canvas-editor-224-verification.md
**Status**: Ready for Step5 review; implementation not started
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-JOIN

## Intent and repository context
Serially verify actual browser/native behavior, collect named-hardware evidence and finalize only owned Git changes.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No fabricated physical results, global staging, forced pushes, PRs, messages or unrelated formatting.

## Related plans
Previous/dependencies: CE-JOIN. Next: workflow final review.

## Write paths
- editor/src-tauri/gen/apple
- editor/test/browser
- editor/test/canvas/measurement.ts
- design-docs/references/canvas-editor-224-evidence.md
- design-docs/user-qa/pending-editor-questions.md
- impl-plans/README.md
- impl-plans/completed
- impl-plans/active/canvas-editor-224-verification.md

## Shared paths and intended edits
None.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| F1 | test/browser/run.mjs | Not started |
| F2 | test/canvas/measurement.ts and evidence.md | Not started |
| F3 | src-tauri/gen/apple and evidence.md | Not started |
| F4 | impl-plans index/archive and final reconciliation | Not started |

### F1: test/browser/run.mjs
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Use CE-PACKAGE locked local Playwright dependency. Add finite node browser runner serving dist with owned server and Playwright browser, closing both in finally. Use node script direct CLI not unavailable global shim. Browser runner exercises realWebGL rendering pixels/text visibility, keyboard/clipboard permission, undo/redo, drag selection, scroll, resize/DPR, context loss/restore and video replacement; exits nonzero on failures and outputs structured results/screenshots. Existing jsdom excludes browser runner. Record manual JapaneseIME and screen-reader steps separately.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### F2: test/canvas/measurement.ts and evidence.md
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Collect hardware/OS/GPU/browser/DPR/audio route/rate/buffer/build/seed. Workload1MiB20000lines with Japanese/longlines,64 voices,4 synth outputs at1024square,720p video+scope. Warmup30s then5min editing and10min resource cycles; text-alone comparison. Capture JS p95<=8ms/presentation p95<=16.7ms p99<=33.4ms,input p95<=50ms,96MiB GPU cap and all subcaps,heap retained growth<=8MiB,dispose baseline.250ms stall/5min drift, audio underruns. Audible impulse/video onset p95<=33.4ms requires external capture and uncertainty<=10ms; browser timestamps alone are not physical sync proof. Missing equipment/metrics remain pending with named reason.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### F3: src-tauri/gen/apple and evidence.md
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Verify CE-PACKAGE locked shell dependency tree, inspect any dependency drift without regeneration by workers; run host and aarch64AppleIOS library checks. Use locally installed Tauri CLI cargo tauri ios init then cargo tauri ios build --debug only after recording installed version and available signing/device; generated build directory is owned. Record physical iPad Japanese keyboard/touch/VoiceOver/orientation/audio unlock/interruption/route latency separately. Xcode26.6 is present, but physical device/signing not established. Update E6 without erasing historical QA.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### F4: impl-plans index/archive and final reconciliation
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Run broad required checks after join, inspect and repair only task drift, independent implementation review and improve self-review. Update own progress and consolidate dependent logs serially; archive only fully met plans, keep device/evidence incomplete plans active. Patch only task rows in dirty impl-plans/README.md. Stage task hunks only using baseline/intent snapshots, display mandated commit summary, commit conventional detailed message and git push origin main non-force. Finalization cannot mark overall acceptance complete with missing gates.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

## Invariants
- Single editing authority; visible source/feedbackGPU, DOM input/accessibility only.
- Source authority, revision/epoch, UTF conversion and flush-before-write preserved.
- Audio never waits for UI; processing/display clocks remain separate.
- Preserve user dirty hunks, latest predecessor contract and resource/queue ceilings.
- Device and measurement evidence remains incomplete unless actually captured.

## Verification commands and required evidence
- `cd editor && npm run check`
- `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm npm run test`
- `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm VACTR_REQUIRE_SESSION_ABI=1 npm run build`
- `node editor/test/browser/run.mjs --mode behavior`
- `node editor/test/browser/run.mjs --mode measure`
- `CARGO_TERM_QUIET=true cargo check`
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
- `CARGO_TERM_QUIET=true cargo fmt -- --check`
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run`
- `CARGO_TERM_QUIET=true cargo check --locked --manifest-path editor/src-tauri/Cargo.toml`
- `CARGO_TERM_QUIET=true cargo fmt --manifest-path editor/src-tauri/Cargo.toml -- --check`
- `CARGO_TERM_QUIET=true cargo check --locked --manifest-path editor/src-tauri/Cargo.toml --lib --target aarch64-apple-ios`
- `CARGO_TERM_QUIET=true cargo tauri ios init`
- `CARGO_TERM_QUIET=true cargo tauri ios build --debug`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [ ] RealGPU behavior evidence, not mocks alone
- [ ] Named hardware measurements and all unavailable gates explicitly recorded
- [ ] Independent review/improve fixes and task-only non-force Git finalization
- [ ] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.
