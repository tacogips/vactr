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
- editor/src/code/format.ts
- editor/src/code/syntax.ts
- editor/src/code/completion-view.ts
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

## Session237 current-baseline preservation amendment
Current main7f6de4e includes formatter shortcut, tree-sitter spans/fallback and context-aware
completion. Accepted15.3 requires coherent migration of existing interactions, not their deletion.
Read current mount.ts, format.ts/format-core.ts, syntax.ts/syntax-core.ts, completion-view.ts,
completion-types.ts, completion-popup.ts and completion.ts immediately before each edit. Core
formatter/syntax/completion implementations and Rust backends are read-only references; retain
Wasm services, language semantics and exact package pins. No new backend, parser or popup system.

### J4: Existing adapter migration (serial before J3 cutover verification)
**Status**: Not started. **Parallelizable**: No; CE-JOIN owns these exact three adapters.
- `editor/src/code/format.ts`: replace EditorView/keymap integration with headless CodeSurface and InputController shortcut wiring. Preserve Shift-Alt-f, Formatter/minimalChange and nonzero/no-change behavior. Suppress composition; capture document revision and text before asynchronous formatting and reject stale response, even if text changed and returned to original. Dispatch accepted edit once through surface with format user event and one undo/history/sync pipeline. No formatter backend change.
- `editor/src/code/syntax.ts`: replace ViewPlugin/Decoration renderer with surface subscriptions and bounded visible GPU style spans from syntax-core styleSpans. Preserve UTF16 capture semantics, class-to-style mapping and fallback vactLanguage tokenization when loader fails. Parse on text revision, reuse on scroll/frame effects, delete replaced/final trees and ignore late loader results after disposal. Mount owns callbacks and passes spans to existing renderer API; no DOM source coloring or hidden EditorView.
- `editor/src/code/completion-view.ts`: adapt existing CompletionSurface to CodeSurface state/subscriptions/dispatch, coordinate mapping and transparent InputController textarea events. Preserve completion-types/CompletionService/CompletionPopup including request sequence/stale text checks, UTF8/16 conversion, popup caret position, blur/composition closure, Ctrl-Space and navigation/accept keys. Dispatch acceptance exactly once with input.complete, retain single sync/history authority. Capture document revision and cursor per request and invalidate on selection/revision changes; same-text restored revisions cannot admit stale results. Use an adapter-local generation wrapper if existing core text guards need reinforcement; do not broaden core backend ownership. Existing DOM completion suggestions are allowed auxiliary panel content; edited source text/cursor/selection/styles stay GPU. Listen to input key events before ordinary navigation only while popup handles the key; respect composition and dispose all listeners.
- `editor/src/code/mount.ts`: wire these adapters and deps.formatter/syntax/completion into one surface; remove legacy extension attach paths; retain async fallback and sample/transport behavior. `editor/src/app/main.ts` preserves service construction; `editor/src/app/apis.ts` preserves source contract, optional service fields stay intact in read-only deps.ts. No CE-CONTRACT replay.
- `editor/test/code/format.test.ts`, `syntax.test.ts`, `syntax-core.test.ts`, `syntax-fallback.test.ts`, `completion-view.test.ts`, `completion-popup.test.ts`, `completion.test.ts`: port only view adapter fixtures to actual headless/input/renderer seam; preserve core assertions. Add direct accepted format/undo/sync exactly-once, stale-revision/IME suppression, fallback/tree disposal, Japanese UTF conversions, popup keyboard/blur/composition/selection rejection, caret scroll/DPR placement and lifetime regressions. Existing test/code directory ownership covers these exact files; new assertions must exercise adapters, not mock their own behavior. Do not weaken existing expectations to achieve a pass.

**Verification**:
- `cd editor && npm run check`: adapter types and optional current deps agree.
- `cd editor && npm run test -- test/code/format.test.ts test/code/syntax.test.ts test/code/syntax-core.test.ts test/code/syntax-fallback.test.ts test/code/completion-view.test.ts test/code/completion-popup.test.ts test/code/completion.test.ts test/canvas/input.test.ts test/canvas/state.test.ts test/canvas/gpu.test.ts`: positive executed/passed counts and zero failures for invariants above, current input/state/GPU combinations.
- Existing J3 combined suite and real-Wasm ABI build remain mandatory with freshly built current artifact; CE-FINAL browser behavior covers actual shortcut/popup/style interaction, not mocked adapters alone.

**Acceptance criteria**:
- [ ] Formatter, syntax/fallback and completion behavior survives GPU cutover; no view authority.
- [ ] Composition, stale revisions/cursors, UTF boundaries, history/sync and disposal regression evidence passes.
- [ ] Current code/app/bind/params/canvas integration and browser behavior pass on source-matched inputs.
Per-edit immutable fresh-read intents/pre/post hashes and lost-intent serial reconciliation follow
execution contract; no parallel adapter writes or shared core edits. Order J1/J2, J4, then J3;
this is four tasks, not additional workflow waves. Required native/formal review gates remain.

### Progress Log: 2026-10-01 — Step4 session237
Current-baseline adapter preservation added from committed7f6de4e source and accepted15.3.2/3.
No implementation edits or tests claimed. Step5 reviews precise scope before plan-only checkpoint.
