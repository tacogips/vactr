# Vactrol

Vactrol is a live-coding language for music and visuals: declarative,
statically typed with inference, written in Rust, targeting the browser
(Wasm), macOS, iPad, and iPhone. See `design-docs/specs/` for the
specification.

## Status

The language front end is implemented as a Rust library
(`impl-plans/active/vactrol-core.md` TASK-001..003):

- `src/value/`: the core value model. It covers value tagging, structural
  equality, dicts as sorted maps (key-ordered iteration and `put`), exact
  self-reducing ratio arithmetic, and int/float widening.
- `src/reader/`: reads `.vact` source into S-expression nodes with
  byte-accurate spans. It covers indentation blocks, `{}` nesting, pipe
  continuation lines, and `#@` directives kept as trivia. `( )` is a
  reader error.
- `src/expand/`: expands `if`/`elif` into `match` and `for` into `map`.
  Its output uses kernel forms only, and malformed forms produce
  diagnostics that point at their origin.

The checker, VM, compiler, pattern engine, scheduler, DSP, session, and
editor/LSP (TASK-004..010) are not implemented yet. Their modules exist
only as the skeleton from `design-implementation.md` section 4.

## Name

A **vactrol** is an electronic component: an LED sealed together with a
photoresistor, so that light controls resistance. It gives Buchla-style
low-pass gates their organic decay and lives inside optical compressors
and tremolos. Light controlling sound is the right picture for a language
that is half patterns and half visuals. The command is `vactrol`; source
files use the `.vact` extension.

**Vactrol**（バクトロール）は電子部品の名前です。LED とフォトレジスタを
一体に封じたもので、光で抵抗を制御します。Buchla 系のローパスゲートに
独特の減衰を与え、光学式コンプレッサーやトレモロの中にも入っています。
「光が音を制御する」という構図を、パターンとビジュアルを半々に持つこの
言語の名前にしました。実行コマンドは `vactrol`、ソースファイルの拡張子は
`.vact` です。

## Development

Tools are managed by mise (`mise install`). Rust is pinned to 1.83.

The CLI belongs to TASK-009. Until then, the binary only prints its version:

```sh
CARGO_TERM_QUIET=true cargo run --bin vactrol
```

Verification:

```sh
CARGO_TERM_QUIET=true cargo build
CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown
CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings
NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 \
  CARGO_TERM_QUIET=true cargo nextest run
CARGO_TERM_QUIET=true cargo test
```

The spec fixtures in `tests/fixtures/spec/manifest.toml` pin reader and
expander output against the code blocks in the spec. To run only those:
`cargo nextest run -E 'binary(spec_fixtures)'`.

Cargo features: `host-native` (the default), `host-wasm`, and `lsp`. For
now they are empty markers. To build for a Wasm host, run
`cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm`.
