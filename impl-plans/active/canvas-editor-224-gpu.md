# CE-GPU: Canvas editor gpu implementation plan

**planId**: CE-GPU
**planPath**: impl-plans/active/canvas-editor-224-gpu.md
**Status**: Ready for Step5 review; implementation not started
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-CONTRACT

## Intent and repository context
Draw all visible source and feedback on WebGL2 with bounded reusable text geometry.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No DOM source renderer, new language operators or consumer migration.

## Related plans
Previous/dependencies: CE-CONTRACT. Next: CE-INPUT, CE-VISUAL.

## Write paths
- editor/src/code/layout.ts
- editor/src/code/atlas.ts
- editor/src/code/renderer.ts
- editor/src/code/resources.ts
- editor/src/code/code.css
- editor/test/canvas/gpu.test.ts
- impl-plans/active/canvas-editor-224-gpu.md

## Shared paths and intended edits
None.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| G1 | code/layout.ts and atlas.ts | Not started |
| G2 | code/resources.ts and renderer.ts | Not started |
| G3 | code/code.css and gpu tests | Not started |

### G1: code/layout.ts and atlas.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Shape visible unwrapped runs via offscreen2D browser fonts, tile within texture limits; tabs four stops and CRLF one visual break. Grapheme navigation maps measured advances to UTF16; never split surrogate/combining clusters. Key atlas by text/font/fallback/DPR/style, invalidate font/DPR. Bound atlas16MiB, layout8MiB; LRU offscreen eviction, bounded batches for visible overflow and long lines without missing text. Report unsupported RTL, test Japanese and ligature caret geometry against shaped output.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### G2: code/resources.ts and renderer.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Concrete allocation ledger shared with visual code: GPU total96MiB, geometry/staging8MiB, canvas/backdrop4M pixels, visual text8MiB, eight1024-square RGBA8 targets, video720p. Reserve transient replacement before allocation; release on resize/dispose/context loss and expose counters. Render background, scrim, selections/playing/eval, syntax text+line numbers, diagnostics, cursor, badges and touch handles in that order. Buffer reuse/dirty uploads only; idle frames do not rebuild text.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### G3: code/code.css and gpu tests
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Style transparent caret bridge without visible duplicate text. Handle context loss/restore from CPU state and empty feedback targets, allocation-failure GPU status with accessible save, effective DPR reduction status. Verify ledger refusal/rollback, retained text after failure, coordinates, clipping, dirty upload counts and resource deletion with recordingGL; realGPU proof is CE-VERIFY.

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
- `cd editor && npm run test -- test/canvas/gpu.test.ts`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [ ] All required annotations have GPU drawing paths
- [ ] Allocation ledger enforces caps and disposal returns live owned bytes to zero
- [ ] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.
