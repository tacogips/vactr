# Architecture Design

This document describes system architecture and design decisions.

## Overview

nagamu is a live-coding scripting language and its LSP server, implemented in Rust.
The surface syntax is indent-based and desugars into S-expressions; the core is a
small Lisp. It runs as an interpreter first and is designed so that a JIT and a
Wasm backend can be added without changing the language.

Design priorities, in order: simplicity, ease of writing, then everything else.

---

## Sections

### Decision Log

| Date | Topic | Decision | Status |
|------|-------|----------|--------|
| 2026-09-24 | Host platform | Own runtime in Rust. Not built on Racket/Rhombus (see `notes.md`). | Decided |
| 2026-09-24 | Live-coding domains | Both music/audio and visuals/graphics are first-class. | Decided |
| 2026-09-24 | Wasm / browser | Definite target. Runtime must stay `wasm32`-compilable. | Decided |
| 2026-09-24 | Typing (v1) | Dynamic, with optional annotations accepted from day one. | Decided |
| 2026-09-24 | Indent rule set | Deferred. See `user-qa/pending-indent-syntax.md`. | Pending |
| 2026-09-24 | Macro hygiene | Unhygienic with auto-gensym (`x#`) proposed; hygiene later if needed. | Proposed |
| 2026-09-24 | Collections | Clojure-style literals (`[]`, `{}`, `:kw`), immutable by default proposed. | Proposed |
| 2026-09-24 | Principle 2 | Syntax flows left to right; only `if`, `match`, and `{}` groups require planning ahead. Library functions take their subject first. | Decided |
| 2026-09-24 | Nesting | `{}` is the only nesting form; `( )` is removed from the surface syntax. | Decided |
| 2026-09-24 | Blocks | `{}` and `:`+indent are the same block; strict vs block parameters (lang-reference QA-54). | Proposed |
| 2026-09-24 | Pairs, dict, struct | `key: value` is sugar for the pair `[:key value]`; a dict IS a list of pairs (`[amp: 0.5 pan: -1]`); `{}` is only ever a block. `struct` = named, closed pair-list with defaults; an enum variant is a struct. Named arguments are pairs. | Decided |
| 2026-09-24 | Pattern matching | `match` with literals, arrays, dicts, guards, or-patterns, and data-carrying enum variants. | Proposed |
| 2026-09-24 | `if` | Sugar over `match` (`false \| nil` clause = else); `when`/`unless`/`elif` likewise; `while` is sugar over `loop`+`match`. | Decided |
| 2026-09-24 | Kernel | Core forms: `match`, `loop`/`break`, `fn`, `let`, `->`, `>:`, `{}`, `enum`. Everything else is a function or sugar. | Proposed |
| 2026-09-24 | Binding `if`/`while` | Zig-style capture: `if subj pattern -> body` binds on presence (nil check); plain `if` tests truthiness. Optionals are nil-able values. | Decided |

### Execution Modes

Live coding requires runtime redefinition; Wasm and JIT want a closed program.
Rather than choosing one, nagamu defines two modes over the same core language.

| Mode | Purpose | `eval` / REPL | Redefinition | Backend |
|------|---------|---------------|--------------|---------|
| Live (default) | Interactive sessions, hot reload | Available | Cheap, per-var | Interpreter / VM / JIT |
| Frozen | Distribution, browser bundles | Not available | None | Typed IR to Wasm or native |

Rule: no core-language feature may make Frozen mode impossible. Features that
only exist in Live mode (e.g. `eval`) live in the runtime, not in the language core.

### Pipeline

```
source text (indent syntax)
  -> reader        : indent rules -> S-expression AST   (also used by LSP/formatter)
  -> expander      : macros -> core forms               (compile time only)
  -> evaluator     : v1 tree-walking, then bytecode VM, then JIT on hot paths
  -> (frozen only) : core forms -> typed IR -> Wasm / native
```

The S-expression AST is the single shared data model: the LSP, formatter,
macros, and both execution modes all consume it. No stage below the reader
sees indentation.

### Redefinition Model

Top-level definitions live in namespaces as late-bound vars (Clojure-style
indirection). Redefining `foo` replaces the var's value; existing callers
observe the new value on their next call. This is what makes live redefinition
cheap and safe, and it costs one pointer load per global call, which the JIT can
remove in Frozen mode.

Errors raised by an expression are reported and discarded; the session and any
running schedule continue. A live session must never die from a user error.

### Time Model

Both target domains are driven by time: audio by beats and samples, graphics by
frames. The core runtime therefore owns one logical clock and scheduler; audio
and graphics are host capabilities that consume it.

```
core: clock (logical time, tempo) + scheduler (at / every / loop)
  |-- audio capability   : synths, samples, MIDI, tempo-synced events
  |-- graphics capability: frame loop, drawing primitives, shader hot-swap
```

Capabilities are traits implemented by the host (native binary or browser).
The core has no direct dependency on any audio or graphics library.

Open question: whether tempo (beats) belongs in the core clock or in the
audio capability. Recorded in the decision log as follow-up.

### Wasm Constraints

- The runtime crate must build for `wasm32-unknown-unknown` (browser) from the
  start; `wasm32-wasi` is a later addition.
- No assumption of OS threads in the core; concurrency is scheduler-driven.
- All I/O and platform access goes through capability traits.
- Live mode in the browser runs the interpreter/VM compiled to Wasm; Frozen
  mode emits Wasm from nagamu programs and is a later milestone.

### Typing

v1 is dynamically typed. Annotations are part of the syntax from the start
and are checked at runtime where cheap, so that programs written today keep
their meaning when a checker or typed IR uses them later. The annotation
syntax is decided together with the indent rule set.

---
