# Vactrol

Vactrol is a live-coding language for music and visuals: declarative,
statically typed with inference, written in Rust, targeting the browser
(Wasm), macOS, iPad, and iPhone. The repository currently contains the
design documents and the Rust scaffold; the language runtime is not yet
implemented. See `design-docs/specs/` for the specification.

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

This repository was initialized from the `rust-v1` template using `ign`. The current binary is a scaffold and prints a greeting:

```sh
CARGO_TERM_QUIET=true cargo run --bin vactrol
```
