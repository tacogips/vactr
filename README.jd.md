# Vactr

[English README](README.md)

Vactr は音楽とビジュアルのためのライブコーディング言語です。宣言的な記法と
型推論を備えた静的型付けを採用し、Rust で実装されています。ブラウザー
（Wasm）、macOS、iPad、iPhone を対象とします。詳しい仕様は
`design-docs/specs/` を参照してください。

## 現在の状態

言語のフロントエンド、中間層、ランタイム、セッション層、エディターの
基盤は実装済みです（`impl-plans/active/vactr-core.md` の TASK-001..010）。
トップレベルの式は読み取り、展開、型検査、コンパイル、実行の順に処理
されます。パターンはオーディオ、MIDI、OSC の出力先へスケジュールされます。
`vactr` コマンドには `repl`、`run`、`serve`、`get`、`lsp` があります。

- `src/value/` は値、構造的等価性、辞書、比率演算、パスと URL を扱います。
- `src/reader/` は `.vact` を位置情報付きの式へ読み取ります。インデント、
  `{}`、パイプ継続行、`#@` ディレクティブに対応します。
- `src/expand/` は `if` / `elif` や `for` をコア構文へ展開します。
- `src/types/` は型推論と静的検査を担当します。
- `src/ns/`、`src/compile/`、`src/vm/` は名前空間、リアクティブな依存関係、
  バイトコード、仮想マシンを実装します。
- `src/pattern/`、`src/clock/`、`src/tex/` はパターン、サイクル時計、
  ビジュアル処理を担当します。
- `src/sched/` はイベントと制御値のスケジュール、MIDI 入力と時計を扱います。
- `src/dsp/` は音声エンジン、ボイス、エフェクト、バス、解析器を実装します。
  音声コールバック内でメモリ確保やロックを行わない設計です。
- `src/host/` はネイティブ、ブラウザー、無音の各ホストを実装します。
- `src/session/` は文書単位の評価、編集世代、JSON プロトコル、REPL を扱います。
- `src/pkg/` は Git タグとローカルディレクトリを利用するパッケージ管理、
  `vactr.toml`、`vactr.lock`、安全なキャッシュ構築を実装します。

コアモジュールは OS スレッドや I/O を使いません。ファイル、ソケット、
子プロセスなどはホスト層、パッケージのネイティブ実装、CLI、LSP に
閉じ込めています。CLI とセッションプロトコルの詳細は
`design-docs/specs/command.md` にあります。

Mutable Instruments の公開音声 DSP を対象とした実装には、動作する
独立設計の適応版と、権利を個別確認したソース段階の移植が混在します。
Plaits の 24 ポジションと Braids の 47 シェイプは実行可能ですが、
これを元のファームウェアと完全に同等な移植とは呼びません。現在の
到達点と残りの作業は
[`impl-plans/active/modular-audio-handoff.md`](impl-plans/active/modular-audio-handoff.md)、
利用したソースと権利表示は [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)
に記載しています。

## エディター

`editor/` は Vite、TypeScript、CodeMirror 6、Solid.js を使用します。
同じ画面が二つの接続方式を扱います。

- ブラウザーでは、Wasm の `Session` がメインスレッドで動作し、音声は
  AudioWorklet で再生されます。
- ネイティブホストでは、`vactr serve` が出力するトークン付き URL を
  `?session=<ws-url>` に指定し、ローカル WebSocket へ接続します。

エディターには再生と評価の操作、テンポと拍の表示、ソース内診断、
パラメーター編集、`#@` ディレクティブ、WebMIDI 学習、ステップ表示、
アナライザー、WebGL2 のビジュアル、サンプルとパッケージの表示があります。
ブラウザー側の自己解析タップ、MIDI 出力、スロットごとのレベル表示は
まだ利用できません。

ブラウザー版の開発手順:

```sh
CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm
cd editor
npm ci
npm run dev
npm run check
npm run test
VACTR_REQUIRE_SESSION_ABI=1 npm run build
```

既定の Wasm 成果物は
`target/wasm32-unknown-unknown/debug/vactr.wasm` です。
`VACTR_WASM` で別の場所を指定できます。`--lib` を付けてビルドすると、
必要な `session_init` を持つライブラリ成果物を生成できます。
`VACTR_REQUIRE_SESSION_ABI=1` は、その ABI がない成果物を拒否します。

`editor/src-tauri/` の Tauri シェルは独立した Cargo クレートです。
ファイルアクセスはダイアログで選択したテキストファイルに制限します。
実機ブラウザーでの音声とビジュアル、Tauri アプリの起動確認は手動確認が
残っています。

## 名前とファイル形式

Vactr の名前は、LED とフォトレジスタを組み合わせて光で抵抗を制御する
光学部品に着想を得ています。Buchla 系のローパスゲートや光学式
コンプレッサーを連想させるこの仕組みが、音楽とビジュアルの両方を扱う
言語の名前の由来です。実行コマンドは `vactr`、ソースファイルの拡張子は
引き続き `.vact` です。

## 開発と実行

開発ツールは mise で管理します。`mise install` を実行してください。
Rust の指定バージョンは 1.83 です。主なコマンド:

```sh
vactr repl [--host native|noop]
vactr run <file.vact> [--host native|noop] [--cycles N]
vactr serve [<file.vact>] [--host native|noop] [--port P] [--bind 127.0.0.1]
vactr get [github.com/<owner>/<name>[@vX.Y.Z]] [--store dir:<root>]
vactr lsp [--session <ws-url>]
```

`--host` の既定値は `native` です。音声デバイスを開けない場合は警告して
`noop` に切り替わります。`run --host noop --cycles N` は仮想時計で
決定的に終了します。`serve` は接続用のトークン付き URL を標準エラーへ
一度だけ出力します。パッケージキャッシュは `$VACTR_HOME/pkg`、未設定時は
`~/.vactr/pkg` に置きます。

チェックアウトから起動する例:

```sh
CARGO_TERM_QUIET=true cargo run --bin vactr -- repl --host noop
CARGO_TERM_QUIET=true cargo run --features lsp --bin vactr -- lsp
```

主な検証コマンド:

```sh
CARGO_TERM_QUIET=true cargo build
CARGO_TERM_QUIET=true cargo build --features lsp
CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm
CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings
CARGO_TERM_QUIET=true cargo fmt -- --check
NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run
CARGO_TERM_QUIET=true cargo test
```

仕様フィクスチャは `tests/fixtures/spec/manifest.toml` にあり、読取器、
展開器、型検査器、評価器の動作を確認します。パッケージテストはローカルの
ディレクトリ、HTTP フィクスチャ、Git リポジトリだけを使い、公開
ネットワークにはアクセスしません。

Cargo の機能は `host-native`（既定）、`host-wasm`、`lsp` です。`lsp` は
`host-native` を含みます。ネイティブ専用の依存クレートは Wasm ビルドへ
含まれません。
