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
| [vactrol-core.md](active/vactrol-core.md) | In Progress (TASK-001..008 completed 2026-09-25, TASK-008 audible beep check pending user confirmation; TASK-009..010 not started) | design-docs/specs/design-implementation.md | 2026-09-25 |
| [vactrol-session-contracts.md](active/vactrol-session-contracts.md) | Ready (SS-CONTRACTS, issue #4 TASK-009, wave 1) | design-implementation.md 14.5.2, 14.5.3, 14.5.6, 14.5.9, 14.5.12 | 2026-09-25 |
| [vactrol-session-pkg.md](active/vactrol-session-pkg.md) | Ready (SS-PKG, wave 2) | design-implementation.md 5.7, 14.5.7 | 2026-09-25 |
| [vactrol-session-directives.md](active/vactrol-session-directives.md) | Ready (SS-DIRECTIVES, wave 2) | design-implementation.md 13.5, 14.5.8 | 2026-09-25 |
| [vactrol-session-analysis.md](active/vactrol-session-analysis.md) | Ready (SS-ANALYSIS, wave 2) | design-implementation.md 12.3, 14.5.9 | 2026-09-25 |
| [vactrol-session-core.md](active/vactrol-session-core.md) | Ready (SS-SESSION, wave 3) | design-implementation.md 14.1-14.4, 14.5.4-14.5.6 | 2026-09-25 |
| [vactrol-session-cli.md](active/vactrol-session-cli.md) | Ready (SS-CLI, wave 4) | command.md; design-implementation.md 14.5.10 | 2026-09-25 |
| [vactrol-session-lsp.md](active/vactrol-session-lsp.md) | Ready (SS-LSP, wave 4) | design-implementation.md 14.3, 14.5.11 | 2026-09-25 |
| [vactrol-session-finalize.md](active/vactrol-session-finalize.md) | Ready (SS-FINAL, serial reconciliation, wave 5) | design-implementation.md 14.5.12, 6.5.7 | 2026-09-25 |
| [ss-session-20260925-s183-dispatch.json](active/ss-session-20260925-s183-dispatch.json) | Dispatch manifest for the eight SS plans (issue #4) | - | 2026-09-25 |
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
