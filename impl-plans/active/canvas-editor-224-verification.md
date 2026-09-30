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
Verify CE-PACKAGE locked shell dependency tree, inspect any dependency drift without regeneration by workers; run host and aarch64AppleIOS library checks. Use CE-PACKAGE task-local pinned CLI and the Session235 unsigned simulator integration commands below; device packaging additionally requires recorded signing availability. Generated build directory is owned. Record physical iPad Japanese keyboard/touch/VoiceOver/orientation/audio unlock/interruption/route latency separately. Xcode26.6 is present, but physical device/signing not established. Update E6 without erasing historical QA.

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
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TERM_QUIET=true cargo build --locked --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`: rebuild real session artifact before final Wasm tests/build; record artifact SHA256 and matching source identities.
- `cd editor && npm run check`
- `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm npm run test`
- `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm VACTR_REQUIRE_SESSION_ABI=1 npm run build`
- `node editor/test/browser/run.mjs --mode behavior`
- `node editor/test/browser/run.mjs --mode measure`
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TERM_QUIET=true cargo check`
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TERM_QUIET=true cargo fmt -- --check`
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run`
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TERM_QUIET=true cargo check --locked --manifest-path editor/src-tauri/Cargo.toml`
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TERM_QUIET=true cargo fmt --manifest-path editor/src-tauri/Cargo.toml -- --check`
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TERM_QUIET=true cargo check --locked --manifest-path editor/src-tauri/Cargo.toml --lib --target aarch64-apple-ios`

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

## Session 235 serial reconciliation and compiler evidence
Use actual seven-wave DAG in execution contract; retained candidates never erase GPU repair gates. After GPU acceptance, serially rerun CE-STATE S4 checks and CE-INPUT typecheck/input tests on the reconciled tree, recording retained provenance versus current-source evidence separately. If drift/behavior failures occur, return to named owner; do not archive or manufacture replacement acceptance. Reconcile pre-task ownership from `tmp/canvas-editor-224/root-observations/pre-task-shared-file-inventory.json`; preserve ParamUnit m, MusicDSP/rumble and dirty index hunks.

Global Clippy remains warnings-denied and recorded with its real exit. CE-TELEMETRY scoped baseline proposal requires Step5 and runner review; global lint is never reported passed under that disposition. Any subsequent diagnostic-source change (especially CE-AUDIO audio.rs) requires renewed source-matched review, not inherited byte-identical waiver. Unresolved review exception or behavioral failure blocks final acceptance.

F3 consumes CE-PACKAGE's recorded exact CLI and Rust1.98.1 targets. Replace bare cargo-tauri discovery with task-local binary. From `editor/`, run foreground:
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TERM_QUIET=true ../tmp/canvas-editor-224/CE-PACKAGE/native-tools/cli/bin/cargo-tauri ios init`
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TERM_QUIET=true ../tmp/canvas-editor-224/CE-PACKAGE/native-tools/cli/bin/cargo-tauri ios build --help`: record exact pinned CLI options; if aarch64-sim syntax differs, use documented simulator target and log the literal command/intent before execution, never guess a passing result.
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TERM_QUIET=true ../tmp/canvas-editor-224/CE-PACKAGE/native-tools/cli/bin/cargo-tauri ios build --debug --target aarch64-sim`: execute only after the preceding help confirms syntax.
Before build, wire CE-SHELL ios/AudioSession.swift into generated project inputs without changing the worker's Swift source; inspect generated target membership. Check actual generated scheme with `xcodebuild -list -project <generated-project-path>` then run its unsigned simulator build with `-sdk iphonesimulator CODE_SIGNING_ALLOWED=NO`. Resolve project/scheme from generated files, record literal commands first. Verify shared native host library and Swift glue are included. Generated artifact absence is a real unresolved platform result, not device absence.
Keep separate evidence rows: Rust device library compile; Rust simulator compile; Swift/generated unsigned simulator compile; packaging/signing; simulator run; physical-device run. Run device packaging only when actual signing/tooling permits; simulator compilation does not prove physical iPad input/audio behavior. No physical iPad is available in current operator E6 evidence; manual Japanese IME/touch/VoiceOver/audio routes, performance and physical synchronization remain pending.
- [ ] Fresh reconciled GPU/state/input and remaining behavioral gates pass with formal reviews.
- [ ] Mobile compiler/integration preparation attempted on available SDK; each compile/sign/simulator/device gate has complete log and final outcome or explicit unavailable reason.
- [ ] Final status retains failed global lint, missing physical evidence and all unresolved review decisions; archive only actually complete plans.
### Session: 2026-09-30 — Step4 session235
Bounded final evidence amendment only. No application/hardware verification or Git finalization executed in planning.
