# Implementation Plans

This directory contains implementation plans that translate design documents into actionable implementation specifications.

## Purpose

Implementation plans bridge design documents (what to build) and actual code (how to build). They provide:
- Clear deliverables without code
- Trait and function specifications
- Dependency mapping for concurrent execution
- Progress tracking across sessions

## Directory Structure

```
impl-plans/
├── README.md              # This file
├── active/                # Currently active implementation plans
│   └── <feature>.md       # One file per feature being implemented
├── completed/             # Completed implementation plans (archive)
│   └── <feature>.md       # Completed plans for reference
└── templates/             # Plan templates
    └── plan-template.md   # Standard plan template
```

## File Size Limits

**IMPORTANT**: Implementation plan files must stay under 400 lines to prevent OOM errors.

| Metric | Limit |
|--------|-------|
| Line count | MAX 1000 lines |
| Modules per plan | MAX 8 modules |
| Tasks per plan | MAX 10 tasks |

Large features are split into multiple related plans with cross-references.

## Active Plans

| Plan | Status | Design Reference | Last Updated |
|------|--------|------------------|--------------|
| [vactrol-core.md](active/vactrol-core.md) | In Progress (TASK-001..010 completed; TASK-010 on 2026-09-26, issue #5, evidence `target/fe-logs/ed-final-*-s188-1.log`; pending user confirmation: the TASK-008 beep check, the TASK-009 audible REPL gate, and for TASK-010 the worklet audio, the real-browser visual pane, `cargo tauri build` and the Tauri app run; the project-level completion criteria wait on those gates) | design-docs/specs/design-implementation.md | 2026-09-26 |
| [vactrol-editor-scaffold.md](completed/vactrol-editor-scaffold.md) | Completed (ED-SCAFFOLD, issue #5 TASK-010, wave 1; npm project, protocol client, store, transports, host.js options; holds the common ED contract; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.3, 15.1.4, 15.1.6 | 2026-09-26 |
| [vactrol-editor-wire.md](completed/vactrol-editor-wire.md) | Completed (ED-WIRE, wave 2; Rust G2-G6; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.2 | 2026-09-26 |
| [vactrol-editor-code.md](completed/vactrol-editor-code.md) | Completed (ED-CODE, wave 2; CodeMirror surface, highlighting, transport, samples; `highlight.ts` beats fix by the operator before session 188; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.4, 15.1.5 | 2026-09-26 |
| [vactrol-editor-midi.md](completed/vactrol-editor-midi.md) | Completed (ED-MIDI, wave 2; WebMIDI access, picker, learn, forwarding; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.9 | 2026-09-26 |
| [vactrol-editor-wasm.md](completed/vactrol-editor-wasm.md) | Completed (ED-WASM, wave 3; Rust G1 browser Session over the raw ABI; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.2 G1; command.md "Browser transport" | 2026-09-26 |
| [vactrol-editor-bind.md](completed/vactrol-editor-bind.md) | Completed (ED-BIND, wave 3; slider panel, write-back, directives, persistence; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.6, 13, 13.5 | 2026-09-26 |
| [vactrol-editor-visual.md](completed/vactrol-editor-visual.md) | Completed (ED-VISUAL, wave 3; WebGL2 RenderHost panes, analyzer displays; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.8, 9 | 2026-09-26 |
| [vactrol-editor-params.md](completed/vactrol-editor-params.md) | Completed (ED-PARAMS, wave 4; parameter editors, sampler waveform, display-only grid/roll; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.7, 13.5 | 2026-09-26 |
| [vactrol-editor-pkg.md](completed/vactrol-editor-pkg.md) | Completed (ED-PKG, wave 4; browser package import, fetch driver, OPFS; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.10 | 2026-09-26 |
| [vactrol-editor-tauri.md](completed/vactrol-editor-tauri.md) | Completed (ED-TAURI, wave 4; standalone Tauri shell crate; `cargo check` exit=0 in session 188, `cargo tauri build` pending user confirmation; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; accepted; archived 2026-09-26) | design-implementation.md 15.1.11 | 2026-09-26 |
| [vactrol-editor-finalize.md](completed/vactrol-editor-finalize.md) | Completed (ED-FINAL, serial reconciliation, wave 5; real-wasm criteria/packages tests, full-tree verification and TASK-010 bookkeeping in session 188; test-integrity, adversarial and integration review accepted; final-tree evidence `target/fe-logs/ed-final-*-s188-1.log`; archived 2026-09-26) | design-implementation.md 15.1.12, 6.5.7 | 2026-09-26 |
| [ed-editor-20260926-s186-dispatch.json](completed/ed-editor-20260926-s186-dispatch.json) | Dispatch manifest for the eleven ED plans (issue #5, TASK-010; amended by checkpoints 1c02480 and ea95f1b; all plans implemented, ED-FINAL session 188; manifest unchanged by ED-FINAL) | - | 2026-09-26 |
| [vactrol-backend-contracts.md](completed/vactrol-backend-contracts.md) | Completed (BE-CONTRACTS, issue #3, wave 1; archived 2026-09-25) | design-implementation.md 12.8.2, 12.8.3, 12.8.5, 12.8.10, 12.8.12 | 2026-09-25 |
| [vactrol-backend-sched.md](completed/vactrol-backend-sched.md) | Completed (BE-SCHED, TASK-007, wave 2; archived 2026-09-25) | design-implementation.md 11.2-11.6, 12.8.3 | 2026-09-25 |
| [vactrol-backend-dsp.md](completed/vactrol-backend-dsp.md) | Completed (BE-DSP, TASK-008, wave 2; archived 2026-09-25) | design-implementation.md 12, 16.1, 12.8.8, 12.8.9 | 2026-09-25 |
| [vactrol-backend-inst.md](completed/vactrol-backend-inst.md) | Completed (BE-INST, TASK-008, wave 2; archived 2026-09-25) | design-implementation.md 12.1, 12.8.6, 12.8.7 | 2026-09-25 |
| [vactrol-backend-midi.md](completed/vactrol-backend-midi.md) | Completed (BE-MIDI, TASK-007, wave 3; archived 2026-09-25) | design-implementation.md 11.7, 12.8.12 | 2026-09-25 |
| [vactrol-backend-native.md](completed/vactrol-backend-native.md) | Completed (BE-NATIVE, TASK-008, wave 3; archived 2026-09-25) | design-implementation.md 12.8.10 | 2026-09-25 |
| [vactrol-backend-wasm.md](completed/vactrol-backend-wasm.md) | Completed (BE-WASM, TASK-008, wave 3; archived 2026-09-25) | design-implementation.md 16, 16.1, 12.8.10, 12.8.11 | 2026-09-25 |
| [vactrol-backend-finalize.md](completed/vactrol-backend-finalize.md) | Completed (BE-FINAL, serial reconciliation, wave 4; archived 2026-09-25) | design-implementation.md 6.5.7, 12.8.12 | 2026-09-25 |
| [be-backend-20260925-s181-dispatch.json](completed/be-backend-20260925-s181-dispatch.json) | Dispatch manifest for the eight BE plans (issue #3; BE-CONTRACTS..BE-WASM accepted, BE-FINAL session 186) | - | 2026-09-25 |
| [vactrol-middle-masks.md](completed/vactrol-middle-masks.md) | Completed (ME-MASKS, issue #2, wave 1; archived 2026-09-25) | design-implementation.md 5.5, 7, 7.1.3, 7.1.6 | 2026-09-25 |
| [vactrol-middle-frontend.md](completed/vactrol-middle-frontend.md) | Completed (ME-FRONTEND, wave 2; archived 2026-09-25) | design-implementation.md 6.5.8 | 2026-09-25 |
| [vactrol-middle-check.md](completed/vactrol-middle-check.md) | Completed (ME-CHECK, TASK-004, wave 3; archived 2026-09-25) | design-implementation.md 5.6, 7, 7.1.4 | 2026-09-25 |
| [vactrol-middle-vm.md](completed/vactrol-middle-vm.md) | Completed (ME-VM, TASK-005, wave 3; archived 2026-09-25) | design-implementation.md 5.5-5.7, 8, 13 | 2026-09-25 |
| [vactrol-middle-pattern.md](completed/vactrol-middle-pattern.md) | Completed (ME-PATTERN, TASK-006, wave 3; archived 2026-09-25) | design-implementation.md 9, 10, 11.1, 11.7 | 2026-09-25 |
| [vactrol-middle-reactive.md](completed/vactrol-middle-reactive.md) | Completed (ME-REACTIVE, TASK-005, wave 4; archived 2026-09-25) | design-implementation.md 5.6, 7.1.3 | 2026-09-25 |
| [vactrol-middle-integrate.md](completed/vactrol-middle-integrate.md) | Completed (ME-INTEGRATE, TASK-004..006, wave 5; archived 2026-09-25) | design-implementation.md 7.1.1, 7.1.3, 7.1.7 | 2026-09-25 |
| [vactrol-middle-finalize.md](completed/vactrol-middle-finalize.md) | Completed (ME-FINAL, serial reconciliation, wave 6; archived 2026-09-25) | design-implementation.md 6.5.7, 7.1.7 | 2026-09-25 |
| [me-middle-20260925-s175-dispatch.json](completed/me-middle-20260925-s175-dispatch.json) | Dispatch manifest for the eight ME plans (issue #2; all plans completed, session 183) | - | 2026-09-25 |

## Completed Plans

| Plan | Completed | Design Reference |
|------|-----------|------------------|
| [vactrol-session-contracts.md](completed/vactrol-session-contracts.md) | 2026-09-26 (SS-CONTRACTS, issue #4 TASK-009, wave 1; archived in f345e62) | design-implementation.md 14.5.2, 14.5.3, 14.5.6, 14.5.9, 14.5.12 |
| [vactrol-session-pkg.md](completed/vactrol-session-pkg.md) | 2026-09-26 (SS-PKG, wave 2; archived in f345e62) | design-implementation.md 5.7, 14.5.7 |
| [vactrol-session-directives.md](completed/vactrol-session-directives.md) | 2026-09-26 (SS-DIRECTIVES, wave 2; archived in f345e62) | design-implementation.md 13.5, 14.5.8 |
| [vactrol-session-analysis.md](completed/vactrol-session-analysis.md) | 2026-09-26 (SS-ANALYSIS, wave 2; archived in f345e62) | design-implementation.md 12.3, 14.5.9 |
| [vactrol-session-core.md](completed/vactrol-session-core.md) | 2026-09-26 (SS-SESSION, wave 3; archived in f345e62) | design-implementation.md 14.1-14.4, 14.5.4-14.5.6 |
| [vactrol-session-cli.md](completed/vactrol-session-cli.md) | 2026-09-26 (SS-CLI, wave 4; archived in f345e62) | command.md; design-implementation.md 14.5.10 |
| [vactrol-session-lsp.md](completed/vactrol-session-lsp.md) | 2026-09-26 (SS-LSP, wave 4; archived in f345e62) | design-implementation.md 14.3, 14.5.11 |
| [vactrol-session-finalize.md](completed/vactrol-session-finalize.md) | 2026-09-26 (SS-FINAL, serial reconciliation, wave 5; archived in f345e62) | design-implementation.md 14.5.12, 6.5.7 |
| [ss-session-20260925-s183-dispatch.json](completed/ss-session-20260925-s183-dispatch.json) | 2026-09-26 (SS dispatch manifest, issue #4; unedited since checkpoint 9d6db6e; archived in f345e62) | - |
| [vactrol-frontend-value.md](completed/vactrol-frontend-value.md) | 2026-09-25 (FE-VALUE, TASK-001, wave 1; archived in 850c606) | design-docs/specs/design-implementation.md 5.1-5.4, 6.5.1-6.5.3 |
| [vactrol-frontend-reader.md](completed/vactrol-frontend-reader.md) | 2026-09-25 (FE-READER, TASK-002, wave 2; archived in 850c606) | design-docs/specs/design-implementation.md 5.7, 6.1-6.3, 6.5.4, 6.5.6 |
| [vactrol-frontend-expander.md](completed/vactrol-frontend-expander.md) | 2026-09-25 (FE-EXPAND, TASK-003, wave 3; archived in 850c606) | design-docs/specs/design-implementation.md 6.4, 6.5.5, 6.5.6 |
| [vactrol-frontend-finalize.md](completed/vactrol-frontend-finalize.md) | 2026-09-25 (FE-FINAL, serial reconciliation, wave 4; archived in 850c606) | design-docs/specs/design-implementation.md 6.5.7 |
| [fe-frontend-20260925-s165-dispatch.json](completed/fe-frontend-20260925-s165-dispatch.json) | 2026-09-25 (FE dispatch manifest; closure note added in issue #2) | - |

## Phase Dependencies (for impl-exec-auto)

**IMPORTANT**: This section is used by impl-exec-auto to determine which plans to load.
Only plans from eligible phases should be read to minimize context loading.

### Phase Status

| Phase | Status | Depends On |
|-------|--------|------------|
| 1 | NOT_STARTED | - |
| 2 | BLOCKED | Phase 1 |
| 3 | BLOCKED | Phase 2 |

### Phase to Plans Mapping

```
PHASE_TO_PLANS = {
  1: [
    # Add Phase 1 plan files here
  ],
  2: [
    # Add Phase 2 plan files here
  ],
  3: [
    # Add Phase 3 plan files here
  ]
}
```

## Workflow

### Creating a New Plan

1. Use the `/impl-plan` command with a design document reference
2. Or manually create a plan using `templates/plan-template.md`
3. Save to `active/<feature-name>.md`
4. Update this README with the new plan entry
5. **IMPORTANT**: If plan exceeds 400 lines, split into multiple files

### Working on a Plan

1. Read the active plan
2. Select a subtask to work on (consider parallelization)
3. Implement following the deliverable specifications
4. Update task status and progress log
5. Mark completion criteria as done

### Completing a Plan

1. Verify all completion criteria are met
2. Update status to "Completed"
3. Move file from `active/` to `completed/`
4. Update this README

## Guidelines

- Plans contain NO implementation code
- Plans specify traits, functions, and file structures
- Subtasks should be as independent as possible for parallel execution
- Always update progress log after each session
- **Keep each plan file under 400 lines** - split if necessary
