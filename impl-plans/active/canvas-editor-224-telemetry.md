# CE-TELEMETRY: Canvas editor telemetry implementation plan

**planId**: CE-TELEMETRY
**planPath**: impl-plans/active/canvas-editor-224-telemetry.md
**Status**: Ready for Step5 review; implementation not started
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-CONTRACT

## Intent and repository context
Produce revision-aware scheduled event end times and periodic timestamped transport snapshots on the real session ABI.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No DSP scheduling changes, native shell or frontend clock algorithm.

## Related plans
Previous/dependencies: CE-CONTRACT. Next: CE-AUDIO, CE-CLOCK.

## Write paths
- src/session/protocol.rs
- src/session/codec.rs
- src/session/publish.rs
- src/session/session.rs
- src/host/wasm/session_half.rs
- src/session/tests/publish.rs
- src/session/tests/codec.rs
- editor/test/wasm/canvas-clock.test.ts
- impl-plans/active/canvas-editor-224-telemetry.md

## Shared paths and intended edits
None.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| T1 | session/protocol.rs and codec.rs | Not started |
| T2 | session/publish.rs and session.rs | Not started |
| T3 | wasm/session_half.rs and tests | Not started |

### T1: session/protocol.rs and codec.rs
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Mirror CE-CONTRACT optional fields with defaults and old-client compatibility. Timing emission bounded20Hz independent of tempo changes, file/revision/span unchanged. Preserve dirty MusicDSP protocol hunks; one owner fresh-reads each edit. Validate epoch/sample/end/latency fields without rejecting old wire envelopes.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### T2: session/publish.rs and session.rs
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
playing_wire end_time = scheduled event time plus original e.dur seconds (not recomputed from later BPM). Snapshot uses runtime cycle position and matching host_now; add publisher cadence and epoch state, pause/lost-clock handling and discontinuity invalidation. Avoid allocations/serialization in audio callback; publication on session control side. Maintain subscription routing and existing tempo behavior for old clients.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### T3: wasm/session_half.rs and tests
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Thread browser host sample time into same session timing semantics while preserving raw ABI and TAG_SESSION. RealWasm test init/eval/tick and read actual telemetry: no tempo-change needed for refreshed snapshots, duration unchanged by subsequent BPM, revision source retained, epoch reset and rate ceiling verified. Rust tests cover malformed/legacy records and subscription/cadence.

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
- `CARGO_TERM_QUIET=true cargo check`
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run session::tests::`
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
- `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm npm run test -- test/wasm/canvas-clock.test.ts`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [ ] Real native-session and Wasm telemetry share contracts
- [ ] Periodic timing/end-time/epoch/legacy tests pass
- [ ] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.
