# Vactrol

Vactrol is a live-coding language for music and visuals: declarative,
statically typed with inference, written in Rust, targeting the browser
(Wasm), macOS, iPad, and iPhone. See `design-docs/specs/` for the
specification.

## Status

The language front end, middle end, runtime back end and session layer
are implemented (`impl-plans/active/vactrol-core.md` TASK-001..009).
Every top-level form goes read -> expand -> check -> compile -> run, and
bound patterns are scheduled into audio, MIDI and OSC sinks. The
`vactrol` binary provides `repl`, `run`, `serve`, `get` and `lsp`. The
editor (TASK-010) is not implemented yet.

Front end (TASK-001..003):

- `src/value/`: the core value model. It covers value tagging, structural
  equality, dicts as sorted maps (key-ordered iteration and `put`), exact
  self-reducing ratio arithmetic, int/float widening, and `path`/`url`
  values.
- `src/reader/`: reads `.vact` source into S-expression nodes with
  byte-accurate spans. It covers indentation blocks, `{}` nesting, pipe
  continuation lines, and `#@` directives kept as trivia. `( )` is a
  reader error. Unquoted paths (`./x ../x ~/x /abs/x`) and `scheme://`
  urls are literals; `#` ends a url.
- `src/expand/`: expands `if`/`elif` into `match` and `for` into `map`.
  Its output uses kernel forms only, and malformed forms produce
  diagnostics that point at their origin.

Middle end (TASK-004..006):

- `src/types/`: the static checker with inference, the forcing masks and
  the native signature table. Scopes chain as prelude (read-only) ->
  session -> fn/block: rebinding a name in the same scope is an error,
  shadowing a prelude name is a hint, and shadowing a user-defined parent
  name is a warning. It enforces SOUND FIRST (`n [..] > s :x` and
  `note [..] > s :x` are errors), checks `s` keywords against the
  late-bound `sound-kit` prelude binding, types `load path` and
  `sample path`, and diagnoses a bare field-less enum variant used as a
  binding pattern in `if`/`match`.
- `src/ns/`, `src/compile/`, `src/vm/`: namespaces and tweak slots, the
  reactive dependency graph and the top-level `Evaluator`, the bytecode
  compiler, and the VM with the core and domain natives.
- `src/pattern/`, `src/clock/`, `src/tex/`: the pattern engine and
  signals (the first list-valued step after `s` gives the structure;
  `midi-notes` is a structure-giving step), the cycle clock, and the
  visual chains with their shader and uniform plans.

Core modules use no OS threads and no I/O. Threads, files, sockets and
child processes are used only in `src/host/native/`, `src/pkg/native/`,
`src/cli/` and `src/lsp/`.

Back end (TASK-007..008):

- `src/sched/`: the scheduler and slot table (d1..d9, `slot`, `out`), the
  two-horizon staging buffer with occurrence merge, commit-time control
  cells, slot generations with the two-class control channel, `once`/`at`,
  dry run in Query effect mode, telemetry, MIDI input draining and MIDI
  clock slave/master.
- `src/dsp/`: the DSP engine (voice pool, event ring, graph and bus graph,
  ugen and effect catalogs, synthesis templates, analyzers writing f32
  cells, the granular engine with live-bus capture) with no allocation or
  locking on the audio callback.
- `src/host/`: capability traits and `NoopHost`; `native/` (cpal, timer
  tick, midir, WAV loader) behind the `host-native` feature; `wasm/` (raw
  ABI, control-cell mirror protocol, resource lifecycle) behind `host-wasm`
  with the AudioWorklet glue in `editor/worklet/` and the browser lifecycle
  harness in `editor/dev-harness/`.
- `examples/beep.rs`: one event end to end on the native host
  (`cargo run --example beep`; audible check pending).

Session layer (TASK-009):

- `src/session/`: `Session` wraps the evaluator and the runtime. It holds
  the per-form eval pipeline and the reactive `bindings` publication (one
  batch per completed pass, so no provisional value leaks). It also
  contains the edit-epoch and doc-revision write authority, the
  `ClientMsg`/`ServerMsg` JSON protocol (v1), and the REPL console.
- `src/pkg/`: Go-style packages (`import github.com/owner/name`). It
  covers `vactrol.toml`, `vactrol.lock`, git-tag semver with minimal
  version selection, the canonical length-prefixed sha256 digest,
  archive safety checks (traversal, absolute paths, escaping symlinks,
  case-fold duplicates, size limits) and atomic cache staging. Native
  git and local-directory stores and the `~/.vactrol/pkg` cache live in
  `pkg/native/`. A running session never fetches; `vactrol get` does.
- `src/directives/`: `#@` directive comments. It covers block
  attachment, labels (`#@ name X`, `label:`), addressed `label.param`
  directives, `BindingKey`, and both binding persistence modes.
  `DirectivePersistence` is the default and keeps panel membership and
  MIDI mappings in the source as `#@` comments.
  `ExternalFilePersistence` is optional and uses `<doc>.bindings.json`.
- Self-analysis: `scope :bus n`, `spectrum :bus bins:`, `capture :bus
  cycles` and `render cycles`, plus `rms`/`peak`/`spectrum`/`scope` over
  sample values. The checker types them and the REPL can call them.
  Offline `render` never mutates the live runtime. Taps and render need
  the native tier. The browser tier and builds without `host-native`
  fail the call with `beyond-capability`.
- `src/cli/`: the `vactrol` verbs and the native session socket
  (loopback only, token in the URL, HTTP 401 on a bad token or path, at
  most 8 connections, 1 MiB frames).
- `src/lsp/`: `vactrol lsp` over stdio (tower-lsp, `lsp` feature). It can
  attach to a running `serve` socket with `--session`.

The CLI and the session protocol are specified in
`design-docs/specs/command.md`. The audible REPL check (a REPL-bound
pattern sounding on the native host) is pending user confirmation. Its
automated proxy is a recording-host test.

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

Usage (see `design-docs/specs/command.md` for flags, exit codes and the
protocol):

```sh
vactrol repl [--host native|noop]
vactrol run <file.vact> [--host native|noop] [--cycles N]
vactrol serve [<file.vact>] [--host native|noop] [--port P] [--bind 127.0.0.1]
vactrol get [github.com/<owner>/<name>[@vX.Y.Z]] [--store dir:<root>]
vactrol lsp [--session <ws-url>]    # needs --features lsp
```

`--host` defaults to `native`. If the audio device fails to open, the
CLI warns and falls back to `noop`. `run --host noop --cycles N` uses a
virtual clock and ends deterministically. `serve` prints its connect
URL, token included, once to stderr. The package cache lives under
`$VACTROL_HOME/pkg` (default `~/.vactrol/pkg`). From a checkout:

```sh
CARGO_TERM_QUIET=true cargo run --bin vactrol -- repl --host noop
CARGO_TERM_QUIET=true cargo run --features lsp --bin vactrol -- lsp
```

Verification:

```sh
CARGO_TERM_QUIET=true cargo build
CARGO_TERM_QUIET=true cargo build --features lsp
CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown
CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm
CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings
CARGO_TERM_QUIET=true cargo clippy --all-targets --features lsp -- -D warnings
CARGO_TERM_QUIET=true cargo fmt --check
NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 \
  CARGO_TERM_QUIET=true cargo nextest run
CARGO_TERM_QUIET=true cargo test
CARGO_TERM_QUIET=true cargo test --features lsp --test lsp_smoke
```

The spec fixtures in `tests/fixtures/spec/manifest.toml` pin reader,
expander, checker and evaluation behavior against the code blocks in
`lang-reference.md` and `design-music.md`. Each case is classified as
`positive`, `diagnostic` or `deferred`. No case is deferred now. Blocks
marked `eval_via = "session"` evaluate as one document through
`Session`; the rest use a fresh `Evaluator`. To run only the fixtures:
`cargo nextest run -E 'binary(spec_fixtures)'`. `tests/cli.rs` runs the
CLI verbs against `NoopHost`. `tests/directive_fixtures.rs` covers the
`#@` vocabulary. Package tests use only a local directory store, a local
HTTP fixture and local git repositories, never the public network.

Cargo features:

- `host-native` (the default): cpal audio, midir MIDI, the session socket
  (tungstenite) and its token (getrandom).
- `host-wasm`: the browser host ABI.
- `lsp`: `vactrol lsp` (tower-lsp, tokio). It implies `host-native`.

The native-only crates are in the non-wasm32 target table, so both wasm32
builds stay clean. To build for a Wasm host, run
`cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm`.
