# Pending: front-end questions from TASK-001..003 (2026-09-25)

These questions came up while designing the reader and expander (design
section 6.5 in `design-docs/specs/design-implementation.md`). Each has a
recommendation that the implementation follows until the question is
answered. Where a behavior needs pinning, a fixture in
`tests/fixtures/spec/manifest.toml` fixes it, so an answer shows up as a
visible test change.

## U1. Install the wasm32 target (user decision)

TASK-001 requires `cargo build --target wasm32-unknown-unknown`. Only
`aarch64-apple-darwin` is installed with the mise-managed Rust 1.83.

- Option A: approve `rustup target add wasm32-unknown-unknown`, or the
  mise equivalent. The criterion is then run and checked.
- Option B (the default until answered): leave the toolchain unchanged.
  The criterion stays unchecked with this reason recorded. The core
  stays wasm-safe by construction: no thread, fs, time, net or process
  use in core modules.

## U2. Reconciling a line-initial `>` (author confirmation)

lang-reference section 1 says a line that begins with `>` continues the
previous expression. It also says a `>` in head position of a line is
greater-than. The recommendation, which the implementation follows,
depends on indentation:
- A `>` line indented deeper than the current statement level continues
  the previous statement.
- A `>` line at statement level is a statement headed by greater-than.

Every spec example fits this reading.

## U3. String escapes (author question)

The spec defines `{}` interpolation but no escapes. The recommendation,
which the implementation follows, is `\" \\ \n \t \{ \}`. Any other
backslash escape is the reader diagnostic `bad-escape`.

## U4. Inline `fn` body (author question)

lang-reference section 4 writes `fn kick-sound: :bd-haus`. With typed
header parameters (`fn f a: int`), `name: x` in a header can be either a
typed parameter or the start of an inline body. The recommendation,
which the implementation follows, is to require the block form
(`fn name params:` plus an indented body). The inline form reads as a
pair and is the expander diagnostic `malformed-fn`. It is pinned as an
authority-question fixture.

## U5. Binding `if` with a bare field-less variant (author question)

`if S P -> T` desugars to `false | nil -> else; P -> T` when `P` is a
single identifier or `_`. That form matches the `g` annotation in
lang-reference section 3. When `P` is a bare field-less variant name
(for example `none`), a truthy value that is not `none` becomes a match
failure instead of taking the else branch. The expander cannot tell a
variant name from a binding name. The recommendation, which the
implementation follows, is to accept this edge; write `match` or
`if {= S none}` instead. It is pinned as an authority-question fixture.

## Answers (architect on the author's behalf, 2026-09-25)

- **U1 — answered: Option A.** `wasm32-unknown-unknown` is now installed
  in the mise-managed toolchain (`rustup target add`). Run and check the
  criterion.
- **U2 — confirmed.** The indentation reading is the intended one: a `>`
  line indented deeper than the current statement continues it; a `>` at
  statement level is greater-than. Every reference example is written
  that way. Pin it with a fixture.
- **U3 — confirmed.** Escapes `\" \\ \n \t \{ \}`; anything else is
  `bad-escape`.
- **U4 — answered: block form only.** The offending example in
  `lang-reference.md` section 4 was the spec's mistake and is now written
  in block form (`fn kick-sound:` with an indented `:bd-haus`). `name: x`
  in a header is always a typed parameter; an inline body is
  `malformed-fn`.
- **U5 — accepted for the expander.** The desugaring is syntactic; the
  checker (TASK-004) knows which names are variants (lang-reference,
  "a name resolving to a variant is a variant pattern") and must
  diagnose a bare field-less variant used as the binding pattern of
  `if`. Record that obligation in TASK-004 when it is planned.
