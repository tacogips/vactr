# User Q&A

This directory contains items requiring user confirmation or decision.

## Purpose

Store questions, pending decisions, and items awaiting user approval.

## File Naming Convention

| Prefix | Use Case |
|--------|----------|
| `qa-` | Questions/confirmation items |
| `pending-` | Pending decisions |

## Current Items

- [pending-indent-syntax.md](./pending-indent-syntax.md) - Indent rule set: resolved as superseded by the decided reader rules (2026-09-24)
- [pending-frontend-questions.md](./pending-frontend-questions.md) - TASK-001..003 front end: wasm32 target install, line-initial `>`, string escapes, inline fn body, bare-variant binding if: all answered 2026-09-25 and folded into design section 6.5
- [pending-middle-end-questions.md](./pending-middle-end-questions.md) - TASK-004..006 middle end: M1-M3 (subject overloading, `range` order, `amp` in `inst` headers) answered 2026-09-25; M4 (unstructured sound at a sink), M5 (`#` in a url) and M6 (two `let sound-kit` lines in one spec block) open, with the recommendations followed by default (design sections 6.5.8, 7.1, 10.1)
- [qa-example.md](./qa-example.md) - Example: Database Selection (template example)
- [pending-example.md](./pending-example.md) - Example: CLI Output Format (template example)

## Adding New Items

1. Create a new file with appropriate prefix (`qa-` or `pending-`)
2. Include clear description of the question or decision needed
3. List available options if applicable
4. Update this README.md with a reference to the new item
