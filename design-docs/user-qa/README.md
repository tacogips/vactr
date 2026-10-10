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
- [pending-backend-questions.md](./pending-backend-questions.md) - TASK-007..008 back end (issue #3): B1 (evidence when headless Chrome is blocked), B2 (implicit control names and DSP name overloads in `inst`/`bus` bodies), B3 (`inst drum: sampler ...:` meaning), B4 (effect fidelity in v1), B5 (session socket moved to TASK-009; raw wasm ABI instead of wasm-bindgen): open, recommendations followed by default (design section 12.8)
- [pending-session-questions.md](./pending-session-questions.md) - TASK-009 session layer (issue #4): S1 (`capture` timing), S2 (analyzer sources in v1; `fft`/`amp` source forms), S3 (package asset bank keywords), S4 (overlay values in Directive persistence), S5 (semantic import versioning), S6 (default host fallback): open, recommendations followed by default (design section 14.5)
- [pending-editor-questions.md](./pending-editor-questions.md) - TASK-010 editor (issue #5): E1 (crates.io access for the Tauri shell `cargo check`; conflicts with the npm-only network rule), E2 (browser taps, MIDI out and the raw oscilloscope stay "not available"; meters use analyzer cells), E3 (per-slot level meters), E4 (ExternalFile persistence editor-side), E5 (node version): open, recommendations followed by default (design section 15.1)
- Canvas workflow session 224: [pending-editor-questions.md](./pending-editor-questions.md#e6-canvas-editor-measurement-and-physical-ipad-evidence-2026-09-30) E6 records named measurement hardware, iOS tooling and physical-device evidence gaps (design 15.3).
- [pending-formatter-syntax-questions.md](./pending-formatter-syntax-questions.md) - Formatter and tree-sitter syntax (2026-09-30): F1 (blank-line policy), F2 (missing grammar wasm at editor build), F3 (format keybinding), F4 (console-bound aliases are refused), F5 (repo-wide formatting): open, recommendations followed by default (design-formatter-and-syntax.md)
- [pending-completion-questions.md](./pending-completion-questions.md) - Completion engine, span core and space-indent repair (2026-09-30, session 226): C1 (keywords only after `:`), C2 (signature help deferred), C3 (wasm builtin snapshot), C4 (repair gate allows `empty-block`), C5 (minimum indent unit 2), C6 (popup keys): open, recommendations followed by default (design-completion.md, design-formatter-and-syntax.md 3.9)
- [pending-bass-questions.md](./pending-bass-questions.md) - Bass voices (design-bass-voices.md): BQ1 (automatic previous-note slide via engine handoff), BQ2 (held-key sustain versus kernel-owned `gate-length`), BQ3 (rumble as an example recipe or a template): open, recommendations followed by default
- [pending-song-mode-questions.md](./pending-song-mode-questions.md) - Song-mode production integration (2026-10-03, session 249): SM1 (Index retention work under production default limits), SM2 (runtime first-time execution of unseen seeds is out of scope for static songs): open, recommendations followed by default (design-song-mode.md production integration contract)
- [pending-ui-style-questions.md](./pending-ui-style-questions.md) - Editor UI style (2026-10-05, session 275): UQ1 (accent hue), UQ2 (hush danger treatment), UQ3 (global `[hidden]` rule): open, recommendations followed by default (design-ui-style.md)
- [pending-tuning-questions.md](./pending-tuning-questions.md) - Microtonal tuning and chord performance (2026-10-10, branch wf/fm1-tuning): TQ1 (MIDI microtonal output), TQ2 (default reference pitch), TQ3 (harp plate defaults), TQ4 (live retuning of held var-driven notes): all decided 2026-10-10 as recommended (TQ1 (a), TQ2 (a), TQ3 12 strips from one period below the root, TQ4 yes) (design-tuning-and-strum.md)
- [qa-example.md](./qa-example.md) - Example: Database Selection (template example)
- [pending-example.md](./pending-example.md) - Example: CLI Output Format (template example)

## Adding New Items

1. Create a new file with appropriate prefix (`qa-` or `pending-`)
2. Include clear description of the question or decision needed
3. List available options if applicable
4. Update this README.md with a reference to the new item
