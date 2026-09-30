# CE-STATE: Canvas editor state implementation plan

**planId**: CE-STATE
**planPath**: impl-plans/active/canvas-editor-224-state.md
**Status**: In Progress — native blocked result retained; reconciled-tree retry and formal reviews pending
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-CONTRACT, CE-GPU

## Intent and repository context
Create the single document editing authority, reusing the proven revision and UTF conversion machinery.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No canvas drawing, bridge event wiring or protocol schema edits.

## Related plans
Previous/dependencies: CE-CONTRACT, CE-GPU (retry handoff). Next: CE-INPUT.

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
| S1 | code/surface.ts | Implemented; final type-check pending |
| S2 | code/sync.ts and history.ts | Implemented; final type-check pending |
| S3 | code/language.ts and state tests | Implemented; final type-check pending |
| S4 | Reconciled-tree verification and native/formal gates | Not started |

### S1: code/surface.ts
**Status**: Implemented; final type-check pending
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Implement CodeSurface with one EditorState authority, transaction dispatch, view-independent undo/redo, annotations and subscription cleanup. Dispatch synchronously calls DocumentSync.apply once before subscriptions; selection/feedback changes never increment revisions. Preserve history grouping semantics and immutable snapshots; no hidden EditorView.

**Completion criteria**:
- [x] File-level behavior above implemented with required invariants.
- [x] Targeted tests prove behavior and complete logs show final exits.

### S2: code/sync.ts and history.ts
**Status**: Implemented; final type-check pending
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Replace EditorView update listeners with surface subscription binding while keeping per-revision ChangeSet mapping, touched-span rejection and 200ms DocSync debounce/flush ordering. Limit256 revisions/four indexes, history+undo32MiB and indexes8MiB. Conservatively account retained text/changes; trim complete undo groups and old revisions with visible reduced-depth status, preserve current document and reject stale mappings.

**Completion criteria**:
- [x] File-level behavior above implemented with required invariants.
- [x] Targeted tests prove behavior and complete logs show final exits.

### S3: code/language.ts and state tests
**Status**: Implemented; final type-check pending
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Expose tokenizer spans for GPU styles without using tokenizer to identify sites. Test multi-change transactions, Japanese/emoji UTF round-trips, deletion-boundary mapping, clipboard replacement/undo/redo, selection-only updates and byte-budget eviction. Keep existing semantics for protocol writes.

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
- `cd editor && npm run test -- test/canvas/state.test.ts test/protocol/utf8.test.ts test/protocol/document.test.ts`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. Planning recorded no execution; implementation evidence follows in the progress log.

## Completion criteria
- [x] Exactly-once synchronous revision recording
- [x] History/undo/index ceilings and stale mapping tests pass
- [x] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved. Improve author check performed below; formal review belongs to downstream workflow steps.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.


### Session: 2026-09-30 — CE-STATE Step 6 attempt-1
**Tasks implemented**: S1, S2, S3. Runtime `acceptedPlanIds` admits CE-CONTRACT; design 15.3 and the committed CE-STATE plan match the implemented contract.
**Current status**: source and targeted behavior implemented; final required whole-editor type check is blocked by two errors in concurrently introduced CE-GPU renderer code. No Git mutations, shared manifest/index edits, Rust modifications or GPU owner edits performed.

- `code/surface.ts`: one headless EditorState, synchronous exactly-once DocumentSync.apply before subscriptions, undo/redo through the same pipeline, immutable owner annotations, cleanup and input/renderer attachment seams. Exposes reduced-depth/retained-byte status for downstream presentation.
- `code/history.ts`: 256-revision/4-index count ceilings, 32 MiB combined surface history/undo accounting and 8 MiB cached index accounting; preserve current text, reject evicted/touched/invalid mappings, evict complete undo groups. Oversized UTF conversions remain available as uncached temporary indexes. Pinned CodeMirror 6.11.1 history adapter retains prototype, effects and grouping metadata; public history field reinitialization preserves other state fields.
- `code/sync.ts`: headless observer binding does not apply transactions twice; transitional EditorView extension remains for CE-JOIN. Existing 200ms debounce and client flush-before-write ordering preserved.
- `code/language.ts`: exact UTF-16 tokenizer style spans remain presentation-only.
- `test/canvas/state.test.ts`: 19 behavioral tests, including replacement/undo/redo, Japanese/emoji/tabs/CRLF conversion, multi-change revision/epoch ordering, deletion boundaries, stale transactions, annotations, complete-group eviction/redo, grouping after eviction, immutable prior undo state, inverted effects, excluded source edits, actual 8 MiB cache eviction, disposal and pending debounce cancellation.

**Complete command evidence** (all foreground processes retained until terminal exit):
- `cd editor && npm run check`: check-1 and check-2 exit 0; final `check-final.log` / `check-final.json` exit 1. Final diagnostics: `editor/src/code/renderer.ts:134:114` and `:240:117`, TS2345 number versus inferred literal `4000000`. Those files are explicitly CE-GPU-owned and outside this plan's write paths. This failure is not passing evidence or a baseline-disposition candidate.
- `cd editor && npm run test -- test/canvas/state.test.ts test/protocol/utf8.test.ts test/protocol/document.test.ts`: final exit 0, 3 files, 30 passed, zero failures (`test-final.log` / `test-final.json`). Earlier test-1 had 26 passes; test-2 had 30 passes; those earlier logs are retained.
- `cd editor && npm run test -- test/code`: final exit 0, 9 files, 57 passed, zero failures (`compatibility-final.log` / `compatibility-final.json`). Earlier compatibility-1 also passed 57 tests.
- All logs and metadata: `tmp/canvas-editor-224/CE-STATE/attempt-1/`. `verified-source-before.json`, `verified-source-after.json` and `verification-source-stability.json` prove identical 175-file editor source/test/config identities across the final commands.

**Improve author check**: found that user transaction filters could rewrite internal history reconfiguration into unsynchronized source edits. Fixed by `filter: false` on history-only reconfiguration; regression proves source/history remain equal and revision increments once. Added grouping-after-eviction and actual byte-cache tests. Read-only `/root/state_selfcheck` found no further high/mid implementation issue. Formal integrity/adversarial review is still downstream, not claimed complete.

**Immutable evidence**: per-edit fresh bytes, expected prehash, intended behavior, predecessor contract identity, dirty diff and posthash under `tmp/canvas-editor-224/CE-STATE/attempt-1/edit-*/` and `/private/tmp/vactr-224-implementation/CE-STATE/attempt-1/edit-*/`. `edit-chain-reconciliation.json` records recovered post-edit bytes verified against contemporaneous posthashes and all five source/test pre-node comparisons match the supplied runtime boundary snapshot. Plan-local handoff identities and `author-self-check.json` retain the final source and concrete repair request.

**Blocking repair / resume criterion**: CE-GPU fixes its renderer/resource size parameter type at lines134/240, preserving its resource ceilings; CE-STATE then reruns `cd editor && npm run check` on the reconciled tree and records final source-matched logs. Do not edit CE-GPU files from CE-STATE.
**Remaining downstream work**: formal integrity/adversarial review, serial combined-tree review, CE-JOIN visible migration, CE-FINAL shared records/archive/Git. GPU/device/manual IME/accessibility/performance evidence belongs to its assigned downstream plans and is not inferred from these state tests.

## Session 227 retry contract (integration finding 1)
The renderer typing repair is already present. Historical attempt-1 `check-final.log`
exit 1 remains unchanged; it is not reclassified as passing. Native CE-STATE result
remains blocked until a new implementation attempt passes progress and formal review gates.

### S4: Reverify implemented state on the reconciled tree
**Status**: Not started
**Parallelizable**: No; wait for CE-GPU accepted handoff, then fresh-read and hash state,
renderer and resource files. CE-STATE must not edit GPU files.
Run the two required commands below plus `cd editor && npm run test -- test/code`.
The state/UTF/document tests must exercise real CodeSurface and DocumentSync with one
revision per change, retained undo groups, stale/touched span rejection, Japanese/emoji
UTF conversion and byte ceilings. Compatibility tests retain the original 57 behavioral
assertions; record actual current counts rather than requiring a fixed count.
Capture complete attempt-2 logs, source hashes before/after and exit statuses. If source
drifts, reconcile and rerun affected checks; prior passing logs cannot certify new bytes.
Report these implementation-exercising results through the runner-supplied native
progress gate, then required test-integrity, adversarial and integration reviews.
Author checks or generic smoke results cannot substitute for workflow acceptance.
Do not uncheck historical behavioral evidence; add these new retry criteria instead:
- [ ] Reconciled-tree typecheck, state/UTF/document and code compatibility checks exit 0.
- [ ] Native progress gate accepts actual behavioral counts/results and source identities.
- [ ] Formal workflow review decisions accepted with no unresolved high/mid findings.

### Session: 2026-09-30 — Step4 session 227 amendment
S4 and CE-GPU retry ordering added. Historical exit 1 preserved. No implementation
checks or reviews run in this planning node; S4 and native acceptance remain pending.
