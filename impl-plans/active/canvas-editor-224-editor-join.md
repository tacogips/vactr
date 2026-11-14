# CE-JOIN: Canvas editor join implementation plan

**planId**: CE-JOIN
**planPath**: impl-plans/active/canvas-editor-224-editor-join.md
**Status**: Ready for Step5 review; implementation not started
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-CONSUMERS, CE-VISUAL, CE-SHELL

## Intent and repository context
Perform serial atomic cutover and integrate browser/native boot after all concrete contracts are ready.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No parallel shared-file editing, new editor semantics or unrelated cleanup.

## Related plans
Previous/dependencies: CE-CONSUMERS, CE-VISUAL, CE-SHELL. Next: CE-FINAL.

## Write paths
- editor/src/code/mount.ts
- editor/src/code/eval.ts
- editor/src/code/diagnostics.ts
- editor/src/code/code-view.tsx
- editor/src/app/main.ts
- editor/src/app/apis.ts
- editor/test/code
- editor/test/app/main.test.ts
- impl-plans/active/canvas-editor-224-editor-join.md

## Shared paths and intended edits
- editor/src/app/apis.ts: Successive ownership with CE-CONTRACT. Fresh-read/hash before each edit; predecessor finishes before dependent edit. CE-PACKAGE alone generates initial locks; finalization updates index/archive after join. Workers use locked checks.
- editor/test/code: Successive ownership with CE-CLOCK. Fresh-read/hash before each edit; predecessor finishes before dependent edit. CE-PACKAGE alone generates initial locks; finalization updates index/archive after join. Workers use locked checks.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| J1 | code/mount.ts, code-view.tsx and app/apis.ts | Not started |
| J2 | code/eval.ts and diagnostics.ts | Not started |
| J3 | app/main.ts and integration tests | Not started |

### J1: code/mount.ts, code-view.tsx and app/apis.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Mount one CodeSurface, GPU renderer and input bridge; remove EditorView construction/imports and final legacy CodeApi.view. Bind all subscriptions, coordinate mapping, annotations and sample browser. Each frame share CE-CLOCK display sample, text invalidation separate from effects. Mount order allows late-read background source; disposal releases captures/listeners/frame/bridge/resources. Keep save path accessible under GPU failure.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### J2: code/eval.ts and diagnostics.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Replace view decorations/keymaps/lint gutter with surface range annotations and input callbacks. Preserve full-text eval, form-at-cursor span,200ms flash/error colors,300ms browser check, native eval diagnostics and runtime clear-slot behavior. Composition suppresses eval. Source doc sync runs before any write. Test error and successful flashes and UTF range mapping.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### J3: app/main.ts and integration tests
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Choose explicit native IPC in Tauri native mode without starting Wasm/AudioContext; preserve browser/PWA Wasm and WebSocket development selection. Wire native activation/suspend/close and audio status, resume gesture browser only, core disposal. Port existing code/app tests against concrete surface, ensure no source DOM fallback. Serial repair joined drift using fresh snapshots; rerun bind/params/code/app suite.

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
- `cd editor && npm run test -- test/code test/app test/bind test/params test/canvas`
- `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm VACTR_REQUIRE_SESSION_ABI=1 npm run build`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [ ] EditorView visible/hidden authority removed
- [ ] Browser/native boot, eval/diagnostics and all consumer regressions pass
- [ ] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.
