# Design Notes

This document contains research findings, investigations, and miscellaneous design notes.

## Overview

Notable items that do not fit into architecture or client categories.

---

## Sections

### Why Not Build on Racket / Rhombus (2026-09)

Rhombus was evaluated as a possible base because it already has an
extensible, parenthesis-light syntax (Shrubbery) on top of a Lisp macro
system. Conclusion: borrow ideas, do not adopt the platform.

| Aspect | Rhombus | Relevance to nagamu |
|--------|---------|---------------------|
| Execution path | Rhombus -> Racket -> Racket CS -> Chez Scheme native | Three layers; slow startup, large distribution |
| Performance | Between JavaScript and Ruby on "Are We Fast Yet?" | Acceptable, but not controllable from our side |
| Wasm | No official target; only Chez Scheme has an Emscripten build, which does not cover the Racket runtime | Blocks our browser goal |
| RacketScript | Racket subset to JavaScript, not Wasm; Rhombus depends on the full expander | Not a viable bridge |
| Macros | Very strong | Reference for our expander design |
| Shrubbery notation | Indent/group-based, extensible operators | Reference for the indent rule set; operator precedence is more than we want |

Takeaways adopted into `architecture.md`:

- Own small runtime in Rust, with the S-expression AST fully exposed.
- Keep macros compile-time only; keep `eval` out of the core language
  (Live-mode runtime feature) so a Frozen/Wasm mode stays possible.
- Do not include continuations / `call/cc` in the core.
- Separate GC-managed values from unboxed value types early.

### How Rhombus Writes Conditionals and Maps (2026-09-24)

Checked against the Rhombus repository sources
(`rhombus/rhombus/scribblings/guide/conditional.scrbl` and `guide/map.scrbl`)
because the doc site refuses automated fetches.

| Form | Rhombus | Notes |
|------|---------|-------|
| `if` | `if 1 == 2 \| "same" \| "different"` | Two `\|` alternatives: then, else. No `elif`. |
| `cond` | `cond \| n == 0: 1 \| n == 1: 1 \| ~else: ...` | Each `\|` is test + `:` block; `~else` keyword; error if nothing matches. |
| `match` | `match n \| 0: 1 \| 1: 1 \| _: ...` | Binding patterns; `_` or `~else` as catch-all. |
| multi-clause `fun` | `fun \| fib(0): 1 \| fib(1): 1 \| fib(n): ...` | Fuses definition and match; clauses may differ in arity. |
| `guard` | `guard user_id != "" \| #false` | Early exit from the enclosing block; `guard.let` pattern variant. |
| Map literal | `{"alice": p, "bob": q}` | Braces + `key: value` + commas; shorthand for `Map(["alice", p], ...)`. |
| Empty / typed | `{}`, `Map{...}`, `MutableMap{...}` | Constructor name may prefix the braces. |
| Lookup | `m["alice"]`, `m.maybe["x"]` | Missing key throws; `.maybe` returns `#false`. |
| Extend | `m ++ {"k": v}`, `{& m, "k": v}` | Immutable; `&` splices or binds the rest. |
| Membership | `"alice" in m` | |

Relevance to nagamu:

- Rhombus uses the same `{}` for blocks and maps and disambiguates by
  position. nagamu disambiguates by content (`{key: value}` vs `{expr}`),
  which is simpler for a line-based reader. Recorded in `lang-reference.md`
  section 2 as QA-59.
- `|`-clause syntax for `cond`/`match` is an alternative to `elif` (QA-41)
  worth revisiting once the block rule is settled.
- Rhombus `if` needs no parentheses because it has infix operators and
  `|` delimiters; nagamu gets the same effect from `{}` blocks.

---
