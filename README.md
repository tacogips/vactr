# Nagamu

Nagamu is a planned scripting language. Its implementation currently contains the Rust template scaffold; the language syntax and runtime have not yet been implemented.

## Name

**Nagamu** comes from the classical Japanese verb **詠む（ながむ）**, meaning to draw out one's voice and recite or sing a poem. The name evokes quietly singing a thought to oneself. The planned command is `nagm`, and Nagamu source files will use the `.nagm` extension.

**Nagamu** は、古語の **詠む（ながむ）** に由来します。声を長く引いて詩歌を口ずさむ言葉で、小さな声で歌うイメージを込めました。実行コマンドは `nagm`、ソースファイルの拡張子は `.nagm` を予定しています。

Reference: [日本国語大辞典「詠む」](https://kotobank.jp/word/%E8%A9%A0%E3%82%80-588109).

## Development

This repository was initialized from the `rust-v1` template using `ign`. The current binary is a scaffold and prints a greeting:

```sh
CARGO_TERM_QUIET=true cargo run --bin nagm
```
