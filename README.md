# Vactrol

Vactrol is a live-coding language for music and visuals: declarative,
statically typed with inference, written in Rust, targeting the browser
(Wasm), macOS, iPad, and iPhone. See `design-docs/specs/` for the
specification.

## Status

The language front end, middle end and runtime back end are implemented
as a Rust library (`impl-plans/active/vactrol-core.md` TASK-001..008).
Every top-level form goes read -> expand -> check -> compile -> run, and
bound patterns are scheduled into audio, MIDI and OSC sinks.

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

Core modules use no OS threads and no I/O. File and sample I/O for `load`
and `sample` stay behind the `NoopHost` source loader for now.

The slot table, scheduler, hosts, DSP, session/REPL/LSP, and editor
(TASK-007..010) are not implemented yet.

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
CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm
CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings
CARGO_TERM_QUIET=true cargo fmt --check
NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 \
  CARGO_TERM_QUIET=true cargo nextest run
CARGO_TERM_QUIET=true cargo test
```

The spec fixtures in `tests/fixtures/spec/manifest.toml` pin reader,
expander, checker and evaluation behavior against the code blocks in
`lang-reference.md` and `design-music.md`. Each case is classified as
`positive`, `diagnostic` or `deferred`. Deferred cases need the DSP, host
file I/O or package loading of TASK-008/009; they are checked for no
panic and no abort only. To run only those:
`cargo nextest run -E 'binary(spec_fixtures)'`.

Cargo features: `host-native` (the default), `host-wasm`, and `lsp`. For
now they are empty markers. To build for a Wasm host, run
`cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm`.
