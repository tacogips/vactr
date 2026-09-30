# CE-INPUT: Canvas editor input implementation plan

**planId**: CE-INPUT
**planPath**: impl-plans/active/canvas-editor-224-input.md
**Status**: Ready for Step5 review; implementation not started
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-STATE, CE-GPU

## Intent and repository context
Preserve Japanese composition, clipboard, navigation and touch editing through a transparent bridge.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No visible DOM text or second document authority; no invented IME evidence.

## Related plans
Previous/dependencies: CE-STATE, CE-GPU. Next: CE-CONSUMERS.

## Write paths
- editor/src/code/input.ts
- editor/src/code/keyboard.ts
- editor/src/code/pointer.ts
- editor/src/code/accessibility.ts
- editor/test/canvas/input.test.ts
- impl-plans/active/canvas-editor-224-input.md

## Shared paths and intended edits
None.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| I1 | code/input.ts and accessibility.ts | Not started |
| I2 | code/keyboard.ts | Not started |
| I3 | code/pointer.ts and tests | Not started |

### I1: code/input.ts and accessibility.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Focusable transparent textarea at GPU caret using visual viewport offsets; bounded surrounding window at8192 UTF16 units on grapheme boundaries. Track document window start and selection including outside-window selection; clipboard/select-all always use document state. Compose preedit in GPU without repeated commits; suppress eval/reset during composition; cancel restores original range. Reconcile beforeinput/input/composition ordering exactly once. Freeze conflicting source writes and revalidate on drain after composition. Screen-reader window shift, value and selection must remain coherent; bounded status announcements.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### I2: code/keyboard.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Word/line/document movements, shift extension, select-all, deletion on grapheme boundaries, history commands and caret scroll. Dispatch Mod-Enter/Mod-Shift-Enter/Mod-. to supplied eval/hush callbacks, suppress during composition. Cut/copy/paste platform events, denied clipboard visible error, one undo group per committed composition. No synthetic browser events counted as real IME evidence.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### I3: code/pointer.ts and tests
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Click/caret, double word/triple line selection, drag/autoscroll, long-press touch word selection and GPU selection handles; numeric drag gets first refusal only on recognized eligible site and correct gesture. Scroll gesture preserves caret; release capture on blur/cancel. Resize/orientation/keyboard/DPR maintains selection. Test composition orders/cancel, window crossing, large selection, clipboard, undo, touch cancellation and source-write conflict.

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
- `cd editor && npm run test -- test/canvas/input.test.ts`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [ ] Single composition commit and cancellation behavior tested
- [ ] Keyboard/clipboard/pointer contracts pass; manual IME/VoiceOver gates remain separately pending
- [ ] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.
