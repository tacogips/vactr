> Superseded by the canvas-cutover plans (completed 2026-10-07).

# CE-INPUT: Canvas editor input implementation plan

**planId**: CE-INPUT
**planPath**: impl-plans/completed/canvas-editor-224-input.md
**Status**: In Progress — implementation verified; downstream formal review pending
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
- impl-plans/completed/canvas-editor-224-input.md

## Shared paths and intended edits
None.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| I1 | code/input.ts and accessibility.ts | Implementation verified |
| I2 | code/keyboard.ts | Implementation verified |
| I3 | code/pointer.ts and tests | Implementation verified |

### I1: code/input.ts and accessibility.ts
**Status**: Implementation verified
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Focusable transparent textarea at GPU caret using visual viewport offsets; bounded surrounding window at8192 UTF16 units on grapheme boundaries. Track document window start and selection including outside-window selection; clipboard/select-all always use document state. Compose preedit in GPU without repeated commits; suppress eval/reset during composition; cancel restores original range. Reconcile beforeinput/input/composition ordering exactly once. Freeze conflicting source writes and revalidate on drain after composition. Screen-reader window shift, value and selection must remain coherent; bounded status announcements.

**Completion criteria**:
- [x] File-level behavior above implemented with required invariants.
- [x] Targeted tests prove behavior and complete logs show final exits.

### I2: code/keyboard.ts
**Status**: Implementation verified
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Word/line/document movements, shift extension, select-all, deletion on grapheme boundaries, history commands and caret scroll. Dispatch Mod-Enter/Mod-Shift-Enter/Mod-. to supplied eval/hush callbacks, suppress during composition. Cut/copy/paste platform events, denied clipboard visible error, one undo group per committed composition. No synthetic browser events counted as real IME evidence.

**Completion criteria**:
- [x] File-level behavior above implemented with required invariants.
- [x] Targeted tests prove behavior and complete logs show final exits.

### I3: code/pointer.ts and tests
**Status**: Implementation verified
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Click/caret, double word/triple line selection, drag/autoscroll, long-press touch word selection and GPU selection handles; numeric drag gets first refusal only on recognized eligible site and correct gesture. Scroll gesture preserves caret; release capture on blur/cancel. Resize/orientation/keyboard/DPR maintains selection. Test composition orders/cancel, window crossing, large selection, clipboard, undo, touch cancellation and source-write conflict.

**Completion criteria**:
- [x] File-level behavior above implemented with required invariants.
- [x] Targeted tests prove behavior and complete logs show final exits.

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
- [x] Single composition commit and cancellation behavior tested
- [x] Keyboard/clipboard/pointer contracts pass; manual IME/VoiceOver gates remain separately pending
- [x] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.

### Session: 2026-09-30 19:38 JST — CE-INPUT implementation

Dependency admission: runtime acceptedPlanIds admits CE-STATE and CE-GPU. Read the
committed checkpoint plan at 4fc37414e2f2704f6737f1545071bf506bd9c0f8 and accepted design
15.3.2–15.3.3; implementation is aligned with the single state authority and GPU source.
No findings were assigned to CE-INPUT by runtime reviewFeedback. Sibling findings were
not reclassified or pulled into this branch.

I1–I3 implemented in the five owned TypeScript paths. InputController exposes transient
presentation text, underline annotations and cursor without committing preedit. The
transparent, focusable accessibility bridge keeps an at-most-8192-unit grapheme window,
document offset and outside-window selection. Native beforeinput/input and composition
orders commit once; empty end data, Escape and blur cancel. Overlapping source writes
remain deferred until commit/cancel and then run their existing revalidation callbacks.
Keyboard navigation, shift extension, full selection, history, grapheme deletion and
supplied evaluation/hush/caret-scroll callbacks are available. Clipboard uses document
selection, supports normalized CRLF/tabs, and reports denial visibly. PointerController
supports click, counted double/triple presses, drag/autoscroll, touch long press/handles,
native scroll without moving caret, eligible numeric first refusal and capture cleanup.

Integration seams are explicit: CE-JOIN owns mounting InputController and PointerController,
geometry bridge, rendering presentation.text and annotations/cursor, GPU handles, scrolling,
and wiring semantic numeric callbacks. CE-CONSUMERS owns consumer migration. This plan
creates no hidden EditorView or new document authority and edits no downstream mount files.

Evidence root: tmp/canvas-editor-224/CE-INPUT/attempt-1/.
Immutable preimage/intent/postimage records: the plan-local edit-* directories, mirrored read-only
under /private/tmp/vactr-224-implementation/CE-INPUT/attempt-1/. edit-chain.json verified
11 source/test edits, chained hashes and current owned postimages before this plan update.
The runtime pre-node snapshot shows five source/test paths absent and the unchanged plan
preimage; no pre-existing input files or user hunks were replaced. All writes were allowlisted;
no worker staging, commits, pushes, worktrees, shared manifest, index or lock rewrites.

Commands and complete terminal evidence:
- cd editor && npm run check — final exit 0; check-final.log and check-final.json.
- cd editor && npm run test -- test/canvas/input.test.ts — final exit 0; 28 run,
  28 passed, zero failed; input-final.log and input-final.json.
- git diff --check -- editor/src/code/input.ts editor/src/code/keyboard.ts
  editor/src/code/pointer.ts editor/src/code/accessibility.ts editor/test/canvas/input.test.ts
  impl-plans/active/canvas-editor-224-input.md — exit 0; diff-check.log and diff-check.json.
Final check/test metadata and source-before/source-after identities show a stable shared
editor source during each command. Required source identities also match across the final
check and test. No detached command or running terminal session remains.

Historical failure retained: input-1.log and input-1.json — exit 1; 25 run, 24 passed,
one failed. Adjacent typing test exposed CodeMirror's unconditional compose history joining;
fixed with input.type.compose.start and full history isolation. input-2.log retained exit 0,
26/26. Improve self-review subsequently fixed CRLF caret offsets using state.toText/Text.length
and added CRLF and oversized-combining-cluster regressions; final 28/28 supersedes the earlier
failure without relabeling it. Initial check-1.log retained exit 0.

Read-only independent exploration: /root/input_contract_inspection found no prerequisite
blocker. /root/input_source_review identified real pointerdown.detail=0 click counting;
fixed and regression tested, final inspection found no material findings. Source review is
additional to later formal integrity/adversarial/integration reviews, which remain pending.
Improve author self-review checked changed hunks, production tests, insertion offsets,
composition/history grouping, allowlist and complete command evidence; no unresolved
material findings. independent-review.json and author-self-check.json record those results.

Remaining manual gates: real Japanese IME, VoiceOver/accessibility and physical iPad keyboard,
touch, orientation/DPR evidence are unperformed. Synthetic jsdom tests prove contracts only;
no device, GPU presentation or input-latency measurement is claimed by this branch. Mixed RTL
hit testing remains unsupported by the accepted predecessor layout. Manual device evidence
and measurements belong to downstream verification, and formal reviews/Git finalization
belong to later workflow steps. Overall plan remains active until those reviews reconcile
its final completion record; assigned implementation and behavioral gates are complete.

### Session: 2026-09-30 19:55 JST — adversarial repair (comm-002941)

Runtime still admits CE-STATE and CE-GPU. Reviewed committed CE-INPUT I1/I3 and design
15.3.3: full document selection must reconcile correctly through the bounded bridge.
Formal test integrity of attempt-1 was accepted, but adversarial review rejected its
outside-window fallback: minimal common-prefix/suffix diff discarded replacement text
shared with the projected old selection (20000 a characters replaced with abc became bc).
The prior 28/28 result did not cover that bug and is not evidence that it was repaired.

Localized correction in editor/src/code/input.ts extracts the replacement using the
original projected selection boundaries, stripping only unselected context. It then
replaces the full document selection. This branch precedes unchanged-value handling,
since replacing a long selection with exactly its projected text still changes source.
In-window minimal diff, composition guards, single revision authority and undo remain.
Seven additive production-controller regressions in editor/test/canvas/input.test.ts
cover shared prefix, shared suffix, both, partially clipped forward/backward selections,
unchanged projected replacement and clipped deletion. Existing 28 cases are preserved.

Evidence: tmp/canvas-editor-224/CE-INPUT/attempt-2/; immutable edit-001 (tests), edit-002
(source) and edit-003 (this progress append), mirrored read-only under
/private/tmp/vactr-224-implementation/CE-INPUT/attempt-2/. source-stability.json matches
initial preimages against runtime E8F219B7-5E53-4300-8627-A7053CC48848.json and confirms
174 identical final check/test/current source identities. Untouched owned paths match
the pre-node snapshot. No source edits outside the assigned writePaths or Git mutations.

Complete foreground command evidence:
- cd editor && npm run test -- test/canvas/input.test.ts — regression-before.log exit 1,
  35 run, 29 passed, 6 failed. Retained reproduction of the reviewed failure; not baseline
  disposition evidence. input-final.log and input-final.json subsequently show exit 0,
  35 run, 35 passed, zero failed on repaired source.
- cd editor && npm run check — check-final.log and check-final.json, exit 0.
- git diff --check -- editor/src/code/input.ts editor/test/canvas/input.test.ts
  impl-plans/active/canvas-editor-224-input.md — diff-check.log and diff-check.json, exit 0.
Prior attempt-1 logs, including historical composition undo failure exit 1 (24/25), are
unchanged. No earlier failed command is relabeled passing. No terminal session remains.

Independent read-only /root/fallback_repair_review confirmed the scoped correction and
seven regressions; no remaining material issue. Improve author self-review rechecked
projected boundaries, preserved unselected context, direction, identical-value handling,
production dispatch/history, source identity and complete command evidence. No unresolved
material findings or assigned verification gaps. review-self-check.json records inspection.

Assigned implementation repair is complete. Prior adversarial rejection remains historical;
renewed test-integrity/adversarial/integration decisions are owned by subsequent workflow
steps and are not claimed here. Manual Japanese IME, VoiceOver, physical device and GPU
presentation evidence remain downstream and unperformed. This node does not resolve
sibling CE-TELEMETRY or CE-PACKAGE gates, edit the shared manifest, stage, commit or push.
