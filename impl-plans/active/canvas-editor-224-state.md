# CE-STATE: Canvas editor state implementation plan

**planId**: CE-STATE
**planPath**: impl-plans/active/canvas-editor-224-state.md
**Status**: Ready for Step5 review; implementation not started
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-CONTRACT

## Intent and repository context
Create the single document editing authority, reusing the proven revision and UTF conversion machinery.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No canvas drawing, bridge event wiring or protocol schema edits.

## Related plans
Previous/dependencies: CE-CONTRACT. Next: CE-INPUT.

## Write paths
- editor/src/code/surface.ts
- editor/src/code/sync.ts
- editor/src/code/history.ts
- editor/src/code/language.ts
- editor/test/canvas/state.test.ts
- impl-plans/active/canvas-editor-224-state.md

## Shared paths and intended edits
None.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| S1 | code/surface.ts | Not started |
| S2 | code/sync.ts and history.ts | Not started |
| S3 | code/language.ts and state tests | Not started |

### S1: code/surface.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Implement CodeSurface with one EditorState authority, transaction dispatch, view-independent undo/redo, annotations and subscription cleanup. Dispatch synchronously calls DocumentSync.apply once before subscriptions; selection/feedback changes never increment revisions. Preserve history grouping semantics and immutable snapshots; no hidden EditorView.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### S2: code/sync.ts and history.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Replace EditorView update listeners with surface subscription binding while keeping per-revision ChangeSet mapping, touched-span rejection and 200ms DocSync debounce/flush ordering. Limit256 revisions/four indexes, history+undo32MiB and indexes8MiB. Conservatively account retained text/changes; trim complete undo groups and old revisions with visible reduced-depth status, preserve current document and reject stale mappings.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### S3: code/language.ts and state tests
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Expose tokenizer spans for GPU styles without using tokenizer to identify sites. Test multi-change transactions, Japanese/emoji UTF round-trips, deletion-boundary mapping, clipboard replacement/undo/redo, selection-only updates and byte-budget eviction. Keep existing semantics for protocol writes.

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
- `cd editor && npm run test -- test/canvas/state.test.ts test/protocol/utf8.test.ts test/protocol/document.test.ts`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [ ] Exactly-once synchronous revision recording
- [ ] History/undo/index ceilings and stale mapping tests pass
- [ ] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.
