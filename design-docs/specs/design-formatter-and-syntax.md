# Formatter and Editor Syntax (tree-sitter, WASM)

This document designs two deliverables. The first is the `.vact` source
formatter that the specs already promise: `architecture.md` "Pipeline",
design-implementation 6.3 and 6.5.4 "Layout" ("the formatter normalizes
such lines to `L + 1`"), and lang-reference section 1 ("tabs, one per
level ... the formatter normalizes"). The second is a tree-sitter
grammar for `.vact` that runs as WASM in the web editor. It also records
the answer to the user's question of 2026-09-30 (section 2).

Status: Implemented and verified 2026-09-30 on `wf/syntax-fmt`; plans
FST-10..40 are in `impl-plans/completed/`. The questions that stay open are in
`design-docs/user-qa/pending-formatter-syntax-questions.md`. Their
recommendations are followed by default.

Follow-up amendment (2026-09-30, session 226, status: Design). This run
adds three things:

- the space-indent repair, section 3.9, with gate changes in 3.1, 3.6
  and 3.8;
- the CodeMirror-free span and format cores, section 5.5;
- the `VACTR_WASM` loader fix of the wasm format test, section 5.4.

The delivery waves are in `design-completion.md` section 8. The open
choices are in `design-docs/user-qa/pending-completion-questions.md`
(C4, C5).

## Overview

| Part | Where | Role |
|------|-------|------|
| Formatter core | `src/fmt/` (new) | A pure `&str -> Formatted` function, clean for wasm32. It changes only layout whitespace and preserves semantics. |
| Formatter surfaces | `src/cli/fmt.rs`, `src/lsp/analysis.rs`, `src/host/wasm/fmt_abi.rs`, `editor/src/code/format.ts` | `vactr fmt`, LSP `textDocument/formatting`, a raw wasm export, and an editor command |
| Grammar | `tree-sitter-vact/` (new, top level) | A syntax used only by the editor. C parser plus a C external scanner, highlight queries and a corpus, built to `tree-sitter-vact.wasm` |
| Editor syntax | `editor/src/code/syntax.ts` (new) | Highlighting with web-tree-sitter. The existing `StreamLanguage` mode stays as the fallback. |

The Rust reader (`src/reader/`) stays the only source of truth for the
language. The grammar is a lenient SUPERSET for highlighting (5.1). The
`vactr` crate never depends on tree-sitter.

### Baseline this document revises

- design-implementation 14.5.11 "Formatting" decided an interim
  whitespace-only LSP formatter, `lsp::analysis::format_edits`. It strips
  trailing blanks and ensures one final newline, and it leaves `#@` lines
  byte-identical. The formatter of section 3 KEEPS every one of those
  rules as its whitespace subset. It adds indentation normalization and a
  reader gate on top. The LSP now calls the formatter (3.7.2).
- design-implementation 6.3 and 6.5.4 define the layout that the
  formatter mirrors. Nothing in them changes.
- `editor/src/code/language.ts` (design 15.1.5) says "There is no Lezer
  grammar". That stays true. Tree-sitter is added beside the mode, not as
  a Lezer grammar, and the mode remains the fallback.

---

## 1. Scope

In scope: the formatter core and its four surfaces; the `tree-sitter-vact/`
grammar with a C scanner, `highlights.scm` and `folds.scm`, the corpus,
the WASM build and the check over every committed `.vact` file; editor
highlighting through web-tree-sitter with the fallback; mise pins and
tasks.

Out of scope for v1: wrapping long lines; any change to spacing inside a
line (3.3 R6); range formatting; `locals.scm` and `indents.scm` (they
have no consumer, and the Rust LSP owns scoping); npm, cargo or other
language bindings of the grammar; reformatting the committed `.vact`
files (3.8 shows they are already fixed points).

## 2. Answer: is tree-sitter C, and does it run as WASM?

Yes to both. The tree-sitter runtime (`lib/`) and every generated parser
(`src/parser.c`) are plain C11. External scanners are also written in C.
The CLI (`tree-sitter build --wasm`) compiles a grammar and its scanner
into a standalone `.wasm` module. The npm package `web-tree-sitter` is
the runtime compiled to WASM with a JS API (`Parser`, `Language`,
`Query`). It loads that module in browsers and in node.

On this machine (2026-09-30), CLI 0.27.0 (mise `aqua:tree-sitter/tree-sitter`)
built a grammar with a C external scanner without emscripten or docker.
It downloads wasi-sdk and binaryen into `~/.cache/tree-sitter`, and
those are already cached. `web-tree-sitter@0.27.0` in node loaded and
parsed with the result. So no C code is ported by hand. The same C
sources become the WASM artifact.

---

## 3. Formatter

### 3.1 API and the reader gate

- `crate::fmt::format(src: &str) -> Formatted`. `Formatted` carries the
  output `text`, an `outcome` (`Unchanged | Changed | Refused`) and the
  reader `diags`.
- `crate::fmt::format_bytes(input: &[u8]) -> (status, bytes)` is the
  byte-level entry point for the wasm export. It exists so the export can
  be tested on native targets. The statuses are 0 = formatted (the output
  may equal the input), 1 = refused, and 2 = not UTF-8. With status 1 or
  2, the output is the input.
- **Gate.** The formatter first runs `reader::read(src, FileId::new(1),
  &AliasEnv::new())`. A non-console file id makes `_1` an error, as in
  files. The empty alias environment is the one `ns/load.rs` and
  `lsp/analysis.rs` use. If any diagnostic has `Severity::Error`, the
  result is `Refused`: `text` is `src` byte for byte, and the diagnostics
  are returned. The formatter never formats a source that the reader
  rejects. This covers mixed tab/space indentation on code lines
  (`indent-space`), unterminated strings, a qualifier that no import in
  the same document binds, and sources over 4 GiB. **Amended
  (session 226):** there is one exception, the space-indent repair of
  3.9. A source whose only defect is space indentation is converted to
  tabs, and it is formatted only when the converted text passes this
  same gate with zero errors. Every other refusal is unchanged.
- It has no host-native dependencies and no new crates. It uses only
  `reader` (the `pub(crate)` `lexer::lex_line` included) and `types::diag`.
  It builds with `--lib --target wasm32-unknown-unknown
  --no-default-features --features host-wasm`.
- It never panics. It only runs index arithmetic on reader-clean input,
  and every slice comes from lexer offsets or ASCII boundaries.

### 3.2 Line model

The source is split at `\n`. Each physical line keeps its terminator:
`\n`, `\r\n`, or none on the last line. After its leading spaces and
tabs (the INDENT RUN), a line is classified with `lex_line`, as
`layout::one_line` does:

| Kind | Test | Carries |
|------|------|---------|
| Blank | no tokens, no comment | nothing |
| Comment-only | no tokens, a comment | the comment's trivia kind (`Comment` or `Directive`) |
| Code | at least one token | tab level `o`, `starts_with_gt`, `opens_block` (last token is `BlockColon`), whether a trailing comment is a `Directive` |

On reader-clean input, code lines have tabs-only indentation, and their
grouping is exactly the one `layout.rs` computes.

### 3.3 Rules

| # | Rule | Source |
|---|------|--------|
| R1 | Indentation is tabs only, one per level. | lang-reference 1, design 6.3 |
| R2 | A statement's first line sits at its body level `F`. The formatter never moves one: on reader-clean input it is already there. | design 6.5.4 |
| R3 | Every `>` continuation line of a statement at body level `F` is re-indented to `F + 1`. When a continuation line opens a block, the whole block (its lines and the comment-only lines in it) moves with it, so the block body stays at opener + 1. | design 6.5.4 ("normalizes such lines to `L + 1`") |
| R4 | Comment-only lines are re-indented by the binding-preserving rule of 3.5. Their output indentation is tabs only. | this document |
| R5 | Trailing spaces and tabs are stripped from every line, except a line that carries a `Directive` trivia item. That line keeps every byte after its indent run. A whitespace-only line becomes empty. | 14.5.11 (kept) |
| R6 | The formatter never changes text between a line's first and last code or comment byte: token spacing, spacing inside `{}` and `[]`, alignment before a trailing comment, and string contents all stay. Whitespace is significant to the lexer (`-x` versus `- x`, `:kw` after whitespace, the pair colon followed by whitespace), so the minimal choice is to leave it alone. | this document |
| R7 | Blank lines inside the file, leading ones included, are kept as empty lines. Runs are NOT collapsed. Blank lines at the end of the file are removed. | 14.5.11 (kept) |
| R8 | Each line keeps its own terminator (`\n` or `\r\n`). A text with content ends with exactly one terminator: the last content line's own, or `\n` when it had none. A text with no content formats to `""`. | 14.5.11 (kept) |
| R9 | No long-line wrapping in v1. | this document |

The only lines whose indentation changes are continuation lines, lines
inside blocks those lines open, and comment-only lines. Lines are never
added, removed or reordered, except the trailing blank lines of R7.

### 3.4 Code-line levels

This is a single pass over the code lines with a stack of FRAMES. Each
frame is a body `(orig level Lo, formatted level Lf, current statement)`.
The stack starts as `[(0, 0, none)]`. For a code line at level `o`:

1. Pop frames while the top frame's `Lo > o`.
2. If `o == Lo`, the line is a statement's first line. Its formatted
   level is `f = Lf`, and it becomes the top frame's current statement.
3. Otherwise `o > Lo`. On reader-clean input the line is a `>`
   continuation of the current statement, and `f = Lf + 1`.
4. If the line opens a block, push `(o + 1, f + 1, none)`.

On reader-clean input, every line lands in exactly one of these cases.
Once a line is formatted, `o == f` for it, so a second run changes
nothing.

### 3.5 Comment-only lines (binding-preserving indentation)

Own-line `#@` directives bind by indentation (design 13.5, `directives::attach`).
A block of them binds to "the nearest preceding target whose indentation
is no deeper than the block's". The width counts every leading tab and
space as 1 (`directives::attach::Lines`). So moving a comment-only line
can move a directive to another statement. The rule below prevents that.

- **Chain.** After the previous code line, the current statements of the
  frames on the stack, from bottom to top (empty frames skipped), form
  the CHAIN `T0..Tk`. Each `Ti` has an original level `oi` and a
  formatted level `fi`, and both sequences strictly increase. The nearest
  preceding target at indentation `<= w` is always the deepest chain
  element with `oi <= w`. The if-chain case is covered too: a stray
  `elif`/`else` line has the same `o`/`f` as its `if`.
- **Rule.** Let `w` be the comment line's original width (tabs + spaces).
  - With no preceding code line, `r = 0`.
  - Otherwise let `i` be the largest index with `oi <= w`. The upper
    bound is `hi = f(i+1) - 1` when `i < k`, else
    `hi = max(fk, b)`, where `b` is the formatted level of the next code
    line (0 at the end of the file).
  - The output level is `r = min(fi + (w - oi), hi)`, written as `r`
    tabs.
- **Why binding is preserved.** Originally `oi <= w < o(i+1)`. After
  formatting, `fi <= r <= f(i+1) - 1`. For `i = k`, `r >= fk`. So the
  same chain element is selected, and the attach index does not change.
  A directive on a code line binds by its line (`target_on_line`), and
  the formatter moves the whole line. The harness checks this directly
  (3.6 G6, asserted in 3.8).
- **Idempotence.** In a second run, `o = f` for code lines and `w = r`,
  so the same `i` is selected. `r - fi` is already within the bound, and
  `r` is reproduced.
- **Examples.**

| Situation | Before (tabs) | After |
|-----------|---------------|-------|
| Leading comment of a block's first statement | `inst a:` / `\t# c` / `\tbody` | unchanged (`i = 0`, `b = 1`, `r = 1`) |
| Trailing comment of a block before a dedent | `\ts :bd` / `\t# c` / `next` | unchanged (`r = 1`) |
| Stray deep comment after a top-level statement | `let a 1` / `\t\t\t# c` / `let b 2` | `# c` at 0 |
| Comment between a statement and its deep `>` line | `x = foo` / `\t\t\t# c` / `\t\t\t> bar` | both at 1 |
| Directive inside a block opened by a deep continuation | `x`, `\t\t\t> f:`, `\t\t\t\ts`, `\t\t\t\t#@ lfo` | `\t> f:`, `\t\ts`, `\t\t#@ lfo` (still bound to `s`) |
| Space-indented comment line | `\ts :bd` / `    # c` (`w = 4`) / `next` | `\t# c` |

### 3.6 Guarantees

For every input:

- G1: no panic.
- G2: `Refused` means the output is byte-identical to the input.
- G3: when not refused, `print_all(read(out).nodes) == print_all(read(src).nodes)`,
  and `read(out)` has no error.
- G4: `format(out).text == out`, with outcome `Unchanged`.
- G5: the trivia sequence is unchanged. Kinds and order are equal, `#@`
  text is byte-identical, and `#` text is equal up to trailing
  blanks.
- G6: the directive placement from `directives::attach` (attach index and
  `trailing`, per directive) is equal.
- G7: output line `n` differs from input line `n` only in its indent run,
  its trailing blanks or its terminator.

For a REPAIRED input (3.9), G3, G5 and G6 are stated against the
converted text `c`, not against `src`, because `read(src)` has errors.
So `read(out)` has no error,
`print_all(read(out).nodes) == print_all(read(c).nodes)`, and the trivia
and directive placement of `out` equal those of `c`. G1, G4 and G7 hold
unchanged against `src`.

### 3.7 Surfaces

#### 3.7.1 CLI `vactr fmt`

- Synopsis: `vactr fmt [--check] <PATH>...` or `vactr fmt [--check] -`.
  It is in the default build (`src/cli/fmt.rs`, parsed in `cli/args.rs`
  as `Command::Fmt { check, inputs }`).
- The inputs are files, or `-` alone. The following are usage errors
  (exit 2): no input, `-` mixed with paths, `-` given twice, and an
  unknown flag. Directories are not walked. A directory path is an IO
  error.
- For each PATH, the file is read as UTF-8 (anything else is an IO
  error). It is formatted and written back only when the outcome is
  `Changed`. `--check` never writes. It prints each path that would
  change to stdout, one per line.
- For `-`, stdin is read and the output text is written to stdout (the
  input unchanged when refused). With `--check -`, nothing goes to
  stdout.
- A refused input prints its diagnostics to stderr as
  `vactr: <path>:<line>:<col>: <diagnostic>`, and the file is left
  untouched. All inputs are processed.
- Exit code: 1 if any IO error occurred. Otherwise 3 if any input was
  refused. Otherwise 1 if `--check` found an input that would change.
  Otherwise 0. `command.md` records the extended meanings of 1 and 3.
- For tests, the verb's body takes explicit stdin, stdout and stderr
  handles. The tests live in `src/cli/tests/fmt.rs` (temp dirs), and the
  flag parsing tests in `src/cli/tests/args.rs`.

#### 3.7.2 LSP `textDocument/formatting`

- The capability already exists (`document_formatting_provider`).
  `lsp::analysis::format_edits(text)` is re-implemented on
  `fmt::format`. `Refused` gives no edits (`Some(vec![])`).
- The edits line up with lines, as G7 allows. A changed line gets at
  most one edit that replaces its indent run and one that deletes its
  trailing blanks. One tail edit removes trailing blank lines and sets
  the final terminator. Every edit removes and inserts only whitespace,
  which keeps the existing test invariant in `src/lsp/tests/analysis.rs`.
  Ranges use the existing UTF-16 `convert.rs`.
- Tests: the existing `formatting_changes_only_whitespace_and_keeps_directive_lines`
  expectations still hold under R5, R7 and R8. New cases check that
  applying the edits gives exactly `fmt::format(text).text` for every
  committed `.vact` and every formatter fixture, and that a
  reader-error document gets no edits.

#### 3.7.3 Wasm export

- A new `src/host/wasm/fmt_abi.rs` is declared in `host/wasm/mod.rs` and
  so compiles only for `wasm32` + `host-wasm`. It exports:
  - `fmt_source(ptr, len) -> u32`: the status of `format_bytes`.
  - `fmt_out_ptr() -> *const u8` and `fmt_out_len() -> u32`: a
    thread-local output buffer. It stays valid until the next
    `fmt_source` call.
- The input goes through the existing `alloc`/`free`. The export does not
  use the outbox, so it never interleaves with session records, and it
  needs no `*_init`. It is pure and callable on any instance.
- The editor uses a dedicated, lazily instantiated instance (3.7.4).

#### 3.7.4 Editor format command

- `editor/src/code/format.ts` defines a `Formatter` interface with the
  method `format(text) -> {status, text}`. `WasmFormatter` fetches
  `vactr.wasm` (the same dist asset) once. It instantiates the module
  with no imports and calls the 3.7.3 exports. It works in both tiers.
  In the native (Tauri) tier no core instance exists. In the browser
  tier, a separate instance keeps the core's outbox and memory
  untouched.
- `formatDocument(view, formatter)`: on status 0 with a different text,
  it dispatches ONE change that covers the differing middle (the common
  prefix and suffix are kept, so the cursor and history behave), with
  `userEvent: 'format'`. The sync extension sends it like any edit. On
  status 1 or 2, or on a load failure, it does nothing. Reader
  diagnostics are already shown by the diagnostics controller.
- The keybinding is `Shift-Alt-f` (see the user-QA file). `boot()` in
  `editor/src/app/main.ts` supplies the formatter through `EditorDeps`.
  Without one (tests), the command does nothing.

### 3.8 Tests and fixtures

- The harness in `src/fmt/tests/` asserts G1-G7 over every committed
  `.vact` file, via `git ls-files '*.vact'`, which gives 10 files today:
  `examples/*.vact` (9) and `src/prelude/templates.vact`. It also asserts
  `out == src` for them (the FIXED-POINT check), because all 10 are
  already fixed points:
  - no trailing blanks, space indentation or comment-only indentation;
  - every `>` line is at statement level + 1;
  - LF, with a single final newline.

  If an implementation finds otherwise, it reports the finding instead
  of reformatting the file.
- Golden fixtures are `src/fmt/tests/fixtures/<name>.in` and
  `<name>.out`. They do NOT use the `.vact` extension, so the `*.vact`
  checks (the fixed-point check, the grammar parse-all check and
  `fmt-vact-check`) skip deliberately unformatted inputs. There is at
  least one fixture per topic:
  - `continuation`: deep `>` lines, and a continuation that opens a block;
  - `blocks`;
  - `pairs`: `name: value`, and `slot :drums gain: 0.8:`;
  - `lambda`: `->:`;
  - `interp`: nested strings, `#` and trailing spaces inside strings;
  - `comments`: stray, space-indented and trailing comments;
  - `directives`: own-line at widths 0..6 in a continuation-block region,
    trailing with trailing blanks, and an `if`/`elif` chain;
  - `blank-lines`, `crlf`, `empty`;
  - `mixed-indent` and `reader-error`, where `.out == .in` and the
    outcome is `Refused`.
- A mutation test uses a deterministic xorshift and no new crate. It
  runs about 2000 edits over the fixtures and committed files: insert or
  delete a tab, space, `>`, `:`, `#@`, newline, `"` or `{`. It asserts
  G1 always, and G3-G7 whenever the mutant reads clean.
- Every Rust file stays under 1000 lines. The core is split into
  `src/fmt/mod.rs` (API, gate, assembly), `src/fmt/lines.rs` (the line
  model) and `src/fmt/levels.rs` (3.4 and 3.5). The repair of 3.9 lives
  in `src/fmt/repair.rs`.
- **Amended (session 226).** The mutation test's reader-error branch
  becomes "`Refused` with `text == mutant`, OR repaired: `Changed`,
  `read(out)` has no error, and every code indent run of `out` is tabs
  only". The 3.9 minimum unit of 2 means a single inserted space
  (width 1) never qualifies. The fixtures `mixed-indent` and
  `reader-error` stay `Refused`.

### 3.9 Space-indent repair (amendment, session 226)

A file whose code lines are indented with spaces is refused today. The
reader reports `indent-space` for each such line. It also reports
`empty-block` for every block opener whose body lines are
space-indented, because those lines have zero tabs and so never reach
the block's level (`reader/layout.rs` `body` and `block`). The repair
converts such a file to tabs only when that conversion is unambiguous.

**Preconditions.** All of these must hold, or the source is refused
unchanged with its original diagnostics (G2):

1. **Error gate.** Every `Severity::Error` diagnostic of `read(src)` has
   the code `indent-space` or `empty-block`, and at least one is
   `indent-space`. `empty-block` is allowed only because space
   indentation causes it. Any other code (unterminated string, unclosed
   group, unexpected indentation, an unbound qualifier, and so on)
   refuses the file.
2. **Pure spaces.** No code line and no comment-only line (3.2) has a
   tab in its indent run. A tab inside a run, or tab indentation on some
   lines and space indentation on others, is MIXED and refused. Blank
   and whitespace-only lines are ignored, because R5 empties them.
3. **Unit.** Let `W` be the set of positive indent widths of the code
   lines and comment-only lines. The unit `u` is `min(W)`. It must be at
   least 2 and at most 8, and every width in `W` must be an exact
   multiple of `u`. Otherwise the widths are AMBIGUOUS, and the file is
   refused. For example, widths `{4, 6}` are refused, and `{2, 4, 6}`
   give `u = 2`.

**Conversion.** Each code line and comment-only line of width `k * u`
gets `k` tabs in place of its indent run. No other byte changes.

**Acceptance.** Let `c` be the converted text. If `read(c)` has any
`Severity::Error`, the source is refused with the ORIGINAL diagnostics.
The unit then did not produce a valid structure. For example, a block
body two units deep reads as unexpected indentation.

Otherwise `format(src)` is the normal pipeline (3.2 to 3.5) applied to
`c`. The outcome is `Changed`, `text` is the formatted `c`, and `diags`
are the diagnostics of `read(c)`.

**Why structure is kept.** Dividing widths by `u` keeps the order of
every pair of widths:

- a line deeper than another stays deeper;
- equal widths stay equal;
- a directive's comparison with its target (3.5) keeps its result.

The zero-error re-read is the final guard.

**Surfaces.** The API does not change, so `vactr fmt`, LSP formatting,
the wasm export and the editor command all get the repair through
`fmt::format`. The LSP edits still change only indent runs (G7).

**Tests** (`src/fmt/tests/repair.rs`, fixtures in `src/fmt/tests/fixtures/`):

- `space2` and `space4` are the `blocks`, `continuation` and
  `directives` shapes indented with 2 and 4 spaces. For each, `format`
  equals the tab version's `format` output, `print_all` equals the tab
  version's, and a second run is `Unchanged` (idempotent).
- These are refused byte for byte, with the original diagnostics:
  - `space-ambiguous` (widths 4 and 6);
  - `space-mixed-lines` (tab lines plus space lines);
  - `mixed-indent` (a tab and a space in one run);
  - `space-other-error` (spaces plus an unclosed `{`);
  - a 1-space file.
- A CLI test in `src/cli/tests/fmt.rs` covers the command end to end. A
  4-space file is rewritten to tabs with exit 0. `--check` lists it and
  exits 1. A `space-ambiguous` file is left untouched, prints its
  diagnostics and exits 3.

---

## 4. Tree-sitter grammar (`tree-sitter-vact/`)

### 4.1 Role and the superset rule

- The grammar is an editor syntax for highlighting and folding. It MUST
  parse every reader-valid file without ERROR or MISSING nodes. The check
  runs over every committed `.vact` (4.6). It MAY accept files the reader
  rejects, and error recovery on them is best effort.
- The tree stays FLAT below the statement level. Pipes (`>`), arrows
  (`->`), fallbacks (`?`), negation and operators are tokens inside a
  statement's item sequence, not tree structure. The Rust reader owns the
  real structure. This keeps the grammar free of precedence and conflict
  tables that would have to track `line.rs`.

### 4.2 Layout

| Path | Status |
|------|--------|
| `grammar.js`, `src/scanner.c`, `tree-sitter.json`, `queries/highlights.scm`, `queries/folds.scm`, `test/corpus/*.txt`, `test/highlight-sample.txt` (the `ts-test` query input), `scripts/parse-all.sh` | authored, committed |
| `src/parser.c`, `src/grammar.json`, `src/node-types.json`, `src/tree_sitter/*.h` | generated by CLI 0.27.0, committed (tree-sitter convention) |
| `tree-sitter-vact.wasm`, other build outputs | build outputs, ignored by `.gitignore` (`tree-sitter-vact/*.wasm`) |

There is no `package.json`, and there are no `bindings/`. The grammar is
consumed only as WASM. `tree-sitter.json` declares the grammar `vact`:
scope `source.vact`, file type `vact`, highlights
`queries/highlights.scm`, version 0.1.0, and the repository's license.
`tree-sitter generate` evaluates `grammar.js` with node, which is already
required (node >= 20) by `editor/`.

### 4.3 Tokens

The regular tokens mirror `src/reader/lexer.rs` and `pathlit.rs`. The
implementation copies the character classes from those files. The
corpus pins every form.

| Token | Shape (summary) |
|-------|-----------------|
| `comment` / `directive` | `#` to the end of the line / `#@` to the end of the line (`directive` wins). Both are extras. |
| `identifier` | a letter, then word bytes, with inner `-` segments (`sample-play`). `a->b` lexes as three tokens. |
| `qualified_identifier` | `prefix.name` |
| `keyword` | `:name` |
| `number` | int, float or ratio (`1/4`), with an optional leading `-` as `lexer.rs` `number` allows |
| `path` / `url` | `./ ../ ~/ /` path literals / `scheme://rest` (6.5.8) |
| `string` | `"` (`string_content`, `escape_sequence` `\" \\ \n \t \{ \}`, or `interpolation` `{ items }`) `"`, on one line. A bare `}` is content. |
| `operator` | `+ - * / = < > <= >= .. -> & \| ?` |
| `wildcard` / `console_register` | `_` / `_<digits>` |
| brackets | `{ } [ ]` |

The EXTERNAL tokens come from `src/scanner.c`:

- `_newline`, `_indent`, `_dedent`. They are computed from CODE lines
  only. Blank and comment-only lines never produce them, which mirrors
  `layout.rs`. Indentation counts tabs; a space counts as width 1. Only
  reader-invalid files contain spaces there, so leniency is fine.
- `_continuation`: the break before a line that is deeper than the
  current statement's line and starts with `>` (and not `>=`). A deeper
  line right after a block colon is an `_indent`, never a continuation.
  In that position a `>` is greater-than (design 6.5.4).
- `block_colon`: a `:` followed only by spaces or tabs, then a comment,
  the line end or EOF. It is decided by lookahead. Any other `:` is left
  to the internal lexer: an immediate `:` after a key for a pair, or
  `:name` for a keyword.
- `string_content`: a run of string bytes other than `"`, `\`, `{` and
  newline. It lives in the scanner so that leading whitespace inside a
  string is never taken as extras.

Scanner state is an indent stack (at most 128 entries, `MAX_NESTING`)
plus the level of the current physical line. The pushed block level is
the level of the line that opened the block + 1, so a block opened by a
continuation line nests under it. The state serializes completely into
the 1024-byte buffer. The scanner returns false in error recovery (every
symbol valid). It uses only the libc subset in tree-sitter's wasm stdlib
(`calloc`, `free`, `memcpy`).

### 4.4 Node types (the contract for the queries and tests)

`source_file`, `statement`, `continuation`, `block`, `group` (`{}`),
`list` (`[]`), `pair` (fields `key`, `value`), `string`,
`string_content`, `escape_sequence`, `interpolation`, `comment`,
`directive`, `identifier`, `qualified_identifier`, `keyword`, `number`,
`path`, `url`, `operator`, `wildcard`, `console_register`,
`block_colon`.

A `statement` is its first line's items, then any `continuation`s (each
is `>` plus the items), then an optional `block_colon` + `block`. A
`block` holds statements between `_indent` and `_dedent`. `->:`, `d1:`,
`0.8:` and `:drums:` are an ordinary item followed by `block_colon`.

### 4.5 Queries

- `highlights.scm` uses exactly the captures the editor renders. Each
  maps 1:1 to an existing fallback token class
  (`editor/src/code/language.ts`):

| Capture | Nodes | Editor class |
|---------|-------|--------------|
| `@comment` | `comment` | `vact-tok-comment` |
| `@comment.directive` | `directive` | `vact-tok-directive` |
| `@string` | `string`, `string_content`, `escape_sequence` | `vact-tok-string` |
| `@number` | `number` | `vact-tok-number` |
| `@string.special.symbol` | `keyword` | `vact-tok-keyword` |
| `@string.special.path` | `path`, `url` | `vact-tok-path` |
| `@keyword` | the first `identifier` of a `statement` or a `group` when it is in `HEADS` (`let var upd fn inst bus look master import slot if`) | `vact-tok-head` |
| `@punctuation.bracket` | `{ } [ ]`, including interpolation braces | `vact-tok-bracket` |

- The `#any-of?` list for `@keyword` equals `HEADS`, and an editor test
  asserts it.
- `folds.scm` is `(block) @fold`.

### 4.6 Build and checks (mise)

| Task | Runs | Passes when |
|------|------|-------------|
| `ts-generate` | sha256 of the generated files in `tree-sitter-vact/src`, `tree-sitter generate`, sha256 again | both hash lists are equal (the output is reproducible with 0.27.0; this works before and after the files are committed, and needs no git write access) |
| `ts-test` | `tree-sitter test`; `tree-sitter query queries/highlights.scm` over a corpus sample; `scripts/parse-all.sh` | the corpus passes, the query compiles, and every `git ls-files '*.vact'` file parses with no `ERROR` or `MISSING` (the script fails on either, not only on the CLI exit code) |
| `ts-build-wasm` | `tree-sitter build --wasm -o tree-sitter-vact.wasm` | the file exists and starts with the wasm magic bytes |
| `fmt-vact-check` | `cargo run --quiet -- fmt --check $(git ls-files '*.vact')` | exit 0 |

`mise.toml` `[tools]` gains `"aqua:tree-sitter/tree-sitter" = "0.27.0"`.

---

## 5. Editor integration

### 5.1 Assets

The `vactr-assets` plugin in `editor/vite.config.ts` already emits and
serves `vactr.wasm`. It gains OPTIONAL syntax assets, emitted at the
dist root and served by the dev middleware under the same names:

- `tree-sitter-vact.wasm`: from `$VACTR_TS_WASM`, else
  `../tree-sitter-vact/tree-sitter-vact.wasm`.
- `highlights.scm`: from `../tree-sitter-vact/queries/highlights.scm`.
- The web-tree-sitter runtime wasm: `web-tree-sitter.wasm` as shipped in
  `node_modules/web-tree-sitter` 0.27.0. The implementation confirms the
  file name against the installed package.

When any of these is missing, the build logs a warning and omits it, and
the editor then uses the fallback (user-QA F2). `vactr.wasm` stays
mandatory, as today. The files sit in `dist`, which Tauri serves
(`frontendDist: ../dist`, relative `base: './'`), and the Tauri CSP
already allows `'wasm-unsafe-eval'` and `connect-src 'self'`.

### 5.2 Loading and fallback

- `editor/src/code/syntax.ts` provides `loadVactSyntax(base)`. It
  dynamically imports `web-tree-sitter` (so the package lands in its own
  chunk), then:
  - calls `Parser.init` with `locateFile` resolving the runtime wasm
    against `base`;
  - calls `Language.load(tree-sitter-vact.wasm)`;
  - fetches `highlights.scm` and builds a `Query`.

  Any rejection means the fallback.
- `mount.ts` puts the language in a `Compartment`, initially
  `vactLanguage()` (the StreamLanguage mode, unchanged). If
  `EditorDeps.syntax` (a loader supplied by `boot()`) resolves, the
  compartment is reconfigured to `treeSitterHighlighting(syntax)` plus
  the `commentTokens: { line: '#' }` language data. The code pane's
  `data-syntax` records `tree-sitter` or `fallback`.
- Without a loader (every existing jsdom test), the mode stays exactly as
  it is today.

### 5.3 Highlighting plugin

A `ViewPlugin` holds the parser and tree. On each document change it
reparses the full text. v1 has no `tree.edit` incremental path, which
avoids edit bookkeeping bugs, and parse times at editor sizes are
milliseconds. It runs the query over the visible ranges and emits
`Decoration.mark` with the class from the 4.5 table. When captures cover
the same range, the first pattern in query order wins. Nested ranges nest
the marks. Offsets are UTF-16 code units on both sides (web-tree-sitter
parses JS strings as UTF-16).

### 5.4 Tests (vitest)

- `editor/test/code/syntax.test.ts` runs under `@vitest-environment node`.
  It loads the real runtime wasm from `node_modules` and the grammar wasm
  (`$VACTR_TS_WASM`, else `../tree-sitter-vact/tree-sitter-vact.wasm`).
  It THROWS when a file is missing, like `test/support/wasm.ts`, so the
  test fails rather than skips. It asserts:
  - the parse tree of a sample (statement, continuation, block, pair,
    interpolation);
  - `rootNode.hasError === false` for every committed `.vact`;
  - the capture names for sample tokens;
  - that every capture in `highlights.scm` has a class mapping;
  - that the `@keyword` list equals `HEADS`.
- A jsdom test (`editor/test/code/syntax-fallback.test.ts`) mounts with a
  rejecting loader and asserts `data-syntax="fallback"` and fallback token
  classes. A resolving fake loader flips it to `tree-sitter`.
- The format tests are two files, because vitest picks the environment
  per file:
  - `editor/test/wasm/format.test.ts` (node), with the real `vactr.wasm`
    through `loadVactrWasm()`: `fmt_source` round trips, a continuation
    fixture changes, and a reader error returns status 1 with the input;
  - `editor/test/code/format.test.ts` (jsdom), with a fake `Formatter`:
    `formatDocument` makes one minimal change, does nothing on refusal,
    and `Shift-Alt-f` is bound.
- `language.test.ts` and all other existing tests stay green unchanged.
- **Amended (session 226, D).** `editor/test/wasm/format.test.ts` reads
  its artifact bytes only through the shared loader. Its private
  `readWasm()`, which hardcodes
  `../target/wasm32-unknown-unknown/debug/vactr.wasm` (line 23), is
  replaced by the path that `loadVactrWasm()` resolves (`wasm.path`, so
  `$VACTR_WASM` wins), or it is removed. No other path to the artifact
  remains in the file.

### 5.5 CodeMirror-free cores (amendment, session 226, B)

The GPU canvas editor (design-implementation 15.3 on `origin/main`)
draws syntax itself and cannot use `Decoration`s. So the tree-sitter
logic that produces style spans moves into a core with no CodeMirror
dependency, and the CodeMirror side becomes a thin adapter.

- `editor/src/code/syntax-core.ts` (new) must not import
  `@codemirror/*`. It holds:
  - `SyntaxCapture`, `ParsedVact`, `VactSyntax`, `SyntaxLoader`,
    `CAPTURE_CLASSES`, `loadVactSyntax` and `createVactSyntax`, moved
    unchanged;
  - `styleSpans(parsed, from, to): StyleSpan[]` (new), with
    `StyleSpan = { from, to, cls }`. It does what `decorations()` does
    today without the `Decoration`: map through `CAPTURE_CLASSES`, drop
    empty and unmapped captures, remove duplicate `(from, to)` pairs
    with the first query pattern winning, and sort by
    `(from, to)`. Offsets are UTF-16. Callers query by range, so a
    renderer can ask for only its visible ranges.

  Reparsing stays full-text, as in 5.3. An incremental `tree.edit`
  path is not added.
- `editor/src/code/syntax.ts` keeps `treeSitterHighlighting`. It
  becomes a thin adapter: for each visible range it calls `styleSpans`
  and wraps each span in `Decoration.mark({ class: cls })`. It
  re-exports every moved symbol, so `main.ts`, `deps.ts` and the
  existing tests keep their imports, and no wiring file changes.
  Highlighting output is identical.
- `editor/src/code/format-core.ts` (new) must not import
  `@codemirror/*`. It holds:
  - `Formatter`, `FormatResult` and `WasmFormatter`, moved;
  - `minimalChange(before, after): { from, to, insert } | null`, the
    common-prefix and common-suffix computation that `formatDocument`
    inlines today.

  `WasmFormatter` accepts either a URL, the constructor it has today, or
  a shared `ToolWasm` (`editor/src/code/tool-wasm.ts`, owned by the same
  plan and specified in `design-completion.md` 6.4).
  `editor/src/code/format.ts` keeps `FORMAT_KEY`, `formatDocument`
  (now calling `minimalChange`) and `formatKeymap`, and it re-exports
  the moved symbols.
- Tests:
  - `editor/test/code/syntax-core.test.ts` (node) loads the real runtime
    and grammar wasm, as `syntax.test.ts` does. It asserts that
    `styleSpans` gives `vact-tok-head`, `vact-tok-keyword`,
    `vact-tok-string`, `vact-tok-comment` and `vact-tok-bracket` spans
    at the expected UTF-16 offsets for a sample that includes Japanese
    text in a comment. Two checks show that the core does not load
    `@codemirror/view`:
    - a `vi.mock('@codemirror/view', ...)` factory that throws;
    - a source-text check that `syntax-core.ts` and `format-core.ts`
      contain no `@codemirror/` import.
  - The existing `syntax.test.ts`, `syntax-fallback.test.ts` and both
    `format.test.ts` files stay green. The jsdom `format.test.ts` adds
    `minimalChange` cases.

---

## 6. Toolchain and supply chain

- mise pins `aqua:tree-sitter/tree-sitter` 0.27.0. No emscripten, no
  docker. The CLI's wasi-sdk and binaryen downloads go to
  `~/.cache/tree-sitter`.
- The editor adds exactly `"web-tree-sitter": "0.27.0"` (a pinned
  dependency) and updates `editor/package-lock.json` through `npm install`.
- No Rust crate is added. Tree-sitter is not a dependency of `vactr`.

## 7. Delivery waves and path ownership

Fanout manifests list only these tracked paths. They never list
`target/`, `editor/node_modules`, `editor/dist`,
`tree-sitter-vact/node_modules`, `tmp/` or `*.wasm`, and the fanout
snapshot stays under 512 entries. `EditorDeps` (`editor/src/app/deps.ts`)
gains two optional fields: `syntax` (the 5.2 loader) and `formatter`
(the 3.7.4 `Formatter`). Existing mounts and tests that omit them are
unaffected.

| Wave | Plan | Write paths | Depends on |
|------|------|-------------|------------|
| 1 | fmt-core | `src/fmt/**` (`mod.rs`, `lines.rs`, `levels.rs`, `tests/**`), `src/lib.rs` | none |
| 1 | ts-grammar | `tree-sitter-vact/**` (4.2 committed files), `mise.toml` (tool pin, `ts-*` tasks and `fmt-vact-check`, so one plan owns the file), `.gitignore` | none |
| 2 | fmt-cli | `src/cli/{args.rs,mod.rs,fmt.rs}`, `src/cli/tests/{mod.rs,args.rs,fmt.rs}` (it runs `mise run fmt-vact-check` as verification) | fmt-core |
| 2 | fmt-lsp | `src/lsp/analysis.rs`, `src/lsp/tests/analysis.rs` | fmt-core |
| 2 | fmt-wasm | `src/host/wasm/{mod.rs,fmt_abi.rs}` | fmt-core |
| 2 | editor-syntax | `editor/src/code/{syntax.ts,mount.ts}`, `editor/src/app/{main.ts,deps.ts}`, `editor/vite.config.ts`, `editor/package.json`, `editor/package-lock.json`, `editor/test/code/{syntax.test.ts,syntax-fallback.test.ts}` | ts-grammar |
| 3 | editor-format | `editor/src/code/{format.ts,mount.ts}`, `editor/src/app/{main.ts,deps.ts}`, `editor/test/code/format.test.ts`, `editor/test/wasm/format.test.ts` | fmt-wasm, editor-syntax (shares `mount.ts`, `main.ts`, `deps.ts`) |
| 4 | closeout | `README.md`, plan status and archive, `impl-plans/README.md` | all |

## 8. Verification gate

Every command runs in the foreground and records its exit status and
full log:

- `CARGO_TERM_QUIET=true cargo build`
- `cargo clippy --all-targets -- -D warnings`, with and without
  `--features lsp`
- `cargo fmt --check`
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run`,
  with and without `--features lsp` (outside the Codex sandbox)
- `cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`,
  then check that the module exports `fmt_source`, `fmt_out_ptr`,
  `fmt_out_len` and `session_init`
- `mise run ts-generate`, `mise run ts-test`, `mise run ts-build-wasm`,
  `mise run fmt-vact-check`
- `cd editor && npm run check && npm test && VACTR_REQUIRE_SESSION_ABI=1 npm run build`,
  then check that `dist/` contains `tree-sitter-vact.wasm`,
  `highlights.scm` and the runtime wasm

## References

See `design-docs/references/README.md` ("Tree-sitter") for the
tree-sitter documentation, the web-tree-sitter package, and the external
scanner guide.
