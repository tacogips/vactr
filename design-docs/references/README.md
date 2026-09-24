# Design References

This directory contains reference materials for system design and implementation.

## External References

### Rust

| Name | URL | Description |
|------|-----|-------------|
| The Rust Book | https://doc.rust-lang.org/book/ | Official Rust programming language book |
| Rust API Guidelines | https://rust-lang.github.io/api-guidelines/ | Rust API design best practices |
| Rust Design Patterns | https://rust-unofficial.github.io/patterns/ | Common Rust design patterns |

### Indent-based Lisp Syntax

| Name | URL | Description |
|------|-----|-------------|
| SRFI-119 (Wisp) | https://srfi.schemers.org/srfi-119/srfi-119.html | Minimal whitespace-to-list rules |
| SRFI-110 (Sweet-expressions) | https://srfi.schemers.org/srfi-110/srfi-110.html | Wisp-like rules plus `f(x)` and `{infix}` |
| Rhombus / Shrubbery | https://docs.racket-lang.org/shrubbery/ | Group-based extensible notation used by Rhombus |
| Rhombus paper | https://doi.org/10.1145/3622818 | "A New Spin on Macros without All the Parentheses" (OOPSLA 2023) |

### Live Coding and Runtime Design

| Name | URL | Description |
|------|-----|-------------|
| Sonic Pi | https://sonic-pi.net/ | Music live-coding environment; timing and scheduling model |
| TidalCycles | https://tidalcycles.org/ | Pattern-based music live coding |
| Clojure Vars | https://clojure.org/reference/vars | Late-bound var indirection enabling live redefinition |
| Cranelift | https://cranelift.dev/ | Rust-native code generator for a future JIT backend |
| WebAssembly GC | https://github.com/WebAssembly/gc | GC proposal relevant to the Frozen-mode Wasm backend |

## Reference Documents

Reference documents should be organized by topic:

```
references/
├── README.md              # This index file
├── rust/                  # Rust patterns and practices
└── <topic>/               # Other topic-specific references
```

## Adding References

When adding new reference materials:

1. Create a topic directory if it does not exist
2. Add reference documents with clear naming
3. Update this README.md with the reference entry
