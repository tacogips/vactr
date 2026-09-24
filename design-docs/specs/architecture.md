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
| 2026-09-24 | Typing | Statically typed with inference (principle 4). Annotations optional, Python-style in headers/bindings/fields. Type errors: diagnostics in Live, compile errors in Frozen. `any` explicit. | Decided |
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
| 2026-09-24 | Identifiers | `[a-zA-Z][a-zA-Z0-9]*(-[a-zA-Z0-9]+)*`: no `_`, no leading digit or `-`, ASCII only. `_` and `_<digits>` are language tokens; operators are a separate non-user-definable class. | Decided |
| 2026-09-24 | Errors | No error type in the language. A failure unwinds to the top level or the current loop iteration, is reported with origin (source, loop, beat), and the session continues. No try/catch, no user-raised errors in v1. nil = absent (`?T`, `x ? d`). `x ! d` optional fallback proposed. | Decided |
| 2026-09-24 | nil (Clojure) | Option is nil (`?T` = T or nil, no wrapper). nil and false are falsy, all else truthy; one rule for plain and binding `if`/`while`. nil-punning on accessors; `and`/`or` return values; `x ? d` == `or x d`. | Decided |
| 2026-09-24 | Pipe | `>` is the pipe (thread-first); a line beginning with `>` continues the previous expression. The `>:` block form is withdrawn. Head-position `>` is greater-than. | Decided |
| 2026-09-24 | `_1` | Console only; a reader error in files. | Decided |
| 2026-09-24 | let / var / upd | `let` immutable, `var` mutable, `upd` reassigns a var. No shadowing in any direction. Values immutable; vars are rebound. Top-level var replaces Sonic Pi `set`/`get`. | Decided |
| 2026-09-24 | Named arguments | `name: value` pairs at call sites; keyword parameters with `= default` in fn headers. | Decided |
| 2026-09-24 | Numbers | Fixed widths: `int` i32, `int64`, `float` f32, `float64`, `ratio` int64/int64. Literals adapt to context. Implicit widening only; explicit narrowing; overflow fails. | Decided |
| 2026-09-24 | Strings | `{}` interpolates inside every string literal. | Proposed |
| 2026-09-24 | Kernel (revised) | `match`, `loop`/`break`, `fn`, `let`, `var`, `upd`, `->`, `{}`, `enum`. | Proposed |
| 2026-09-24 | Principle 3 | Readable live code over type safety: no rule may lengthen a chain for a static guarantee. | Decided |

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

### Realtime Validation

Errors such as division by zero are exceptions (see `lang-reference.md`,
errors). Surfacing them before and while code plays is a tooling concern,
not a language one. Three layers, each independent of the next:

| Layer | When | Where | Catches |
|-------|------|-------|---------|
| Static diagnostics | while typing | LSP | literal `/ x 0`, undefined names, unknown sample/synth keywords, `match` missing an enum variant, loop body without `sleep`/`sync`, annotation mismatches |
| Dry run before swap | on eval of a live-loop or pattern | runtime | any exception raised by one iteration (or one cycle of a pattern) run against a no-op capability host with the current live state; the old body keeps playing until the dry run passes |
| Runtime diagnostics | while playing | runtime -> LSP | uncaught exceptions from the live session, shown on the originating line with loop name and beat, cleared when the next iteration succeeds |

What the language guarantees to make this possible:

- Every error carries its origin: source position, loop name, beat.
- All effects are capability calls (audio, graphics, I/O), so a
  validation host can implement them as no-ops.
- Pattern evaluation is pure: querying a cycle has no side effects.

Consequences for the toolchain: the LSP server and the live session share
one process (or a socket), because runtime diagnostics originate in the
scheduler. `nagm` therefore runs both; an external editor connects to
the same session the console shows.

### Typing

nagamu is statically typed with inference (principle 4 in
`lang-reference.md`). Every expression has a type the checker can name;
annotations are optional because inference usually makes them
unnecessary. Annotations go in `fn` headers, `let`/`var` bindings,
and struct/enum fields, Python-style (`name: type`, `= default`).

| Mode | Type error |
|------|------------|
| Live | Diagnostic in the editor and console; the code still runs |
| Frozen | Compile error |

`any` is an explicit escape hatch that must be narrowed by a pattern
before use. Numbers have fixed widths: `int` (i32), `int64`, `float` (f32),
`float64`, and exact `ratio` (int64/int64); literals adapt to context.
Widening is implicit (int -> int64, float -> float64, int -> float/float64);
narrowing is an explicit call; overflow is a failure. Keywords are typed like
Zig enum literals against the enum or host set expected at that
position. The inference model (per-function monomorphic vs HM-lite) is
open as QA-92.

---
