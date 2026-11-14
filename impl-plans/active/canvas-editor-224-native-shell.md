# CE-SHELL: Canvas editor shell implementation plan

**planId**: CE-SHELL
**planPath**: impl-plans/active/canvas-editor-224-native-shell.md
**Status**: Ready for Step5 review; implementation not started
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-AUDIO, CE-CLOCK, CE-PACKAGE

## Intent and repository context
Provide buildable shared mobile bootstrap and native Session IPC using the existing host.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No SwiftUI shell, AUv3, generated-code evidence presented as device proof or fallback silently to noop audio.

## Related plans
Previous/dependencies: CE-AUDIO, CE-CLOCK, CE-PACKAGE. Next: CE-JOIN.

## Write paths
- editor/src-tauri/src
- editor/src-tauri/ios
- editor/src-tauri/Cargo.toml
- editor/src-tauri/tauri.conf.json
- editor/src-tauri/capabilities/default.json
- editor/src/protocol/tauri.ts
- editor/test/canvas/tauri.test.ts
- impl-plans/active/canvas-editor-224-native-shell.md

## Shared paths and intended edits
- editor/src-tauri/Cargo.toml: Successive ownership with CE-PACKAGE. Fresh-read/hash before each edit; predecessor finishes before dependent edit. CE-PACKAGE alone generates initial locks; finalization updates index/archive after join. Workers use locked checks.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| N1 | src-tauri/src/lib.rs, main.rs, session.rs and clock.rs | Not started |
| N2 | src-tauri/Cargo.toml, tauri.conf.json, capabilities/default.json and ios/AudioSession.swift | Not started |
| N3 | protocol/tauri.ts and shell tests | Not started |

### N1: src-tauri/src/lib.rs, main.rs, session.rs and clock.rs
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Shared library bootstrap with thin desktop main; register native session open/apply/close and clock_probe commands and output event channel. Construct Session and its Rc instrument registry INSIDE one control thread (never send Rc/Session across threads). Follow src/cli/mod.rs make-session wiring: NativeHosts::open_with_bus_names, shared cells Tier::Native, native rate, sandboxed loader and SessionConfig.insts. Tick using TickSource wake and FrameClock, pass routed replies by conn/seq. Bound requests64 and telemetry64 separately; no lost control replies. Owner stops/joins control thread and tick/audio on close; init failure surfaces capability error.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### N2: src-tauri/Cargo.toml, tauri.conf.json, capabilities/default.json and ios/AudioSession.swift
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Consume CE-PACKAGE root vactr path dependency; add shared library crate outputs/mobile entry without changing dependency versions; constrain invoke permissions to editor window. iOS Swift glue only for AVAudioSession activation/interruption/route changes, communicating invalidate/reopen to native owner and refreshing latency provenance. Wire glue into generated Apple build inputs; CE-PACKAGE owns initial dependency lock generation and CE-FINAL owns generated gen/apple files; no new dependencies in this worker. Reject unsupported iOS host capability visibly; do not replace native path by PWA. Desktop may retain explicitly selected interimWasm mode.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### N3: protocol/tauri.ts and shell tests
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Implement existing Transport over bounded native apply/event commands; subscribe before sending/open, preserve sequence routing, close idempotently, cleanup events on failure. Probe sends/receives page+engine times and metadata. Native tests use headless session injection for open/eval/telemetry/close, failure and queue overflow; frontend tests cover subscriptions and late events after close. Official installed Tauri/CPAL APIs must be checked before coding; no guessed mobile build claim.

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
- `CARGO_TERM_QUIET=true cargo check --locked --manifest-path editor/src-tauri/Cargo.toml`
- `CARGO_TERM_QUIET=true cargo test --locked --manifest-path editor/src-tauri/Cargo.toml --lib`
- `cd editor && npm run test -- test/canvas/tauri.test.ts`
- `CARGO_TERM_QUIET=true cargo check --locked --manifest-path editor/src-tauri/Cargo.toml --lib --target aarch64-apple-ios`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [ ] Desktop/shared-library IPC builds and tests pass
- [ ] iOS compile/sign/device checks tracked individually; unavailable checks unchecked
- [ ] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.
