# Vactrol Front End: Reader and Spec Fixture Manifest (FE-READER) Implementation Plan

**planId**: FE-READER (implements vactrol-core.md TASK-002)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md sections 5.7, 6.1-6.3, 6.5.4, 6.5.6
**Created**: 2026-09-25
**Last Updated**: 2026-09-25 (session 169 revision: design 6.5.7 verification evidence rule)
**Issue**: https://github.com/tacogips/vactrol/issues/1
**dependsOn**: FE-VALUE (wave 2; needs `Span`/`FileId`/`NodeId`, `Ratio64`, `Diagnostic`/`DiagCode`)

---

## Intent and Context

This plan implements the reader: source text in, S-expression `Node` trees out. It covers the
error-recovery diagnostics, the trivia side table (comments and `#@` directives), the two-phase import
frontend and a canonical printer. It also creates the spec fixture manifest and its runner, which
pin the reader behavior against `design-docs/specs/lang-reference.md` and `design-music.md`.
Design section 6.5.4 is normative for every rule below. Where this plan and 6.5.4 differ, 6.5.4 wins and the
difference is reported in the progress log. The reader is fed untrusted text (packages, the editor), so it must
never panic.

## Non-Goals

- No expansion or desugaring. `IfChain`, `Neg`, `Fallback` and `Interp` stay as reader nodes, and FE-EXPAND handles them.
- No interning. The reader stores names as `Rc<str>` text.
- No package fetching, `open` semantics or directive attachment. Directive attachment belongs to the editor (13.5).
- No formatter.
- No settling of the two remaining authority questions (design 6.5.6). They are recorded as fixtures only.
  U2 (indentation reading of a line-initial `>`) and U3 (escapes) are answered and binding; implement them as 6.5.4 states.

## writePaths (exclusive)

- `src/reader/node.rs`, `lexer.rs`, `layout.rs`, `line.rs`, `import.rs`, `sexpr.rs`.
- `src/reader/tests/mod.rs`, `lexer.rs`, `layout.rs`, `line.rs`, `import.rs`, `spans.rs`, `no_panic.rs`.
- `tests/spec_fixtures.rs`, `tests/support/mod.rs`, `tests/support/toml_subset.rs`.
- `tests/fixtures/spec/manifest.toml`.

## sharedPaths

- `src/reader/mod.rs`, created by FE-VALUE. Add the submodules, `read`, `ReadResult` and the re-exports.
  Keep `pub mod span;`.
- `src/types/diag.rs` changes only if a missing code is found. Add it and record the addition in the progress log for FE-FINAL.

## File-Level Changes (signatures only)

- `node.rs`:
  - `Node { pub id: NodeId, pub kind: NodeKind, pub span: Span, pub children: Box<[Node]> }`.
  - `NodeKind { Atom(Atom), Call, List, Block, Pair, Arrow, Splat, Neg, Fallback, Interp, IfChain, Import(Box<ImportDecl>), Error }`.
  - `Atom { Int(i64), Float { value: f64, exact: Option<Ratio64> }, Ratio(Ratio64), Str(Rc<str>), Keyword(Rc<str>), Sym(Rc<str>), Qualified { prefix: Rc<str>, name: Rc<str> }, Op(Op), Wildcard, ConsoleReg(u32), Nil, Bool(bool), Builtin(Rc<str>) }`.
  - `Op { Add, Sub, Mul, Div, Eq, Lt, Gt, Le, Ge, Range, Arrow, Amp, Bar, Question }`, with `as_str`.
  - `Trivia { pub items: Vec<TriviaItem> }`; `TriviaItem { pub span: Span, pub kind: TriviaKind }`; `TriviaKind { Comment, Directive }`.
  - Helpers: `Node::walk(&self, f)`, pre-order; `Node::contains_error() -> bool`.
- `lexer.rs`: a token type (private or `pub(crate)`) carrying kind, span and `preceded_by_space: bool`. The lexer
  implements the 6.5.4 lexer rules: numbers, ranges, qualified names, `_` and console registers,
  stray characters, comments, one-line strings with escapes and `{}` interpolation (nested scan), and the colon classes
  (block opener, pair key, keyword).
- `layout.rs` implements the 6.5.4 layout: tab-counted levels, `indent-space`, statements at body level `L`, a block body at level `B`,
  continuation lines (a `>`-initial line deeper than `L`), `continuation-after-block`, `unexpected-indent`, `empty-block`,
  `IfChain` grouping, and error recovery per statement.
- `line.rs` implements the 6.5.4 statement grammar:
  - `import` (top-level only).
  - `let`/`var`/`upd` as `(head TARGET EXPR-of-rest)`.
  - `fn` as a flat item list with no arrow split.
  - The expression rule: arrow, then pipe, then fallback, then operand.
  - The trailing-block attachment.
  - The item kinds: atom, pair, splat, neg, list, group, and the group-arrow collapse.
  - Head-position `>` (the first token of a segment) is greater-than.
  - The nesting cap is 128 (`nesting-too-deep`).
- `import.rs`:
  - `ImportDecl { pub path: Rc<str>, pub alias: Option<Rc<str>>, pub open: bool, pub prefix: Rc<str>, pub span: Span }`.
  - `AliasEnv { pub prefixes: BTreeMap<Rc<str>, Rc<str>> }`, with `new()`, `bind(prefix, path)` and `is_bound(&str)`.
  - `prescan_imports(src: &str) -> Vec<ImportDecl>`: layout only, skipping malformed imports. The path, prefix and alias rules are those of 6.5.4.
- `mod.rs`:
  - `read(src: &str, file: FileId, env: &AliasEnv) -> ReadResult`, which clones the env and binds each top-level import's prefix after that form.
  - `ReadResult { pub nodes: Vec<Node>, pub diags: Vec<Diagnostic>, pub trivia: Trivia }` with `next_node_id() -> NodeId`.
  - Node ids are sequential in pre-order across the file, starting at 0.
- `sexpr.rs`: `print(&Node) -> String` and `print_all(&[Node]) -> String` (joined with `\n`), exactly per the 6.5.4
  printer table. A float prints in shortest form with `.0` when integral. A string prints quoted and escaped.

## Canonical Expectations (these strings must match exactly)

| Source | `print` after read |
|--------|--------------------|
| `+ 43 32 > * 12 > print` | `(print (* (+ 43 32) 12))` |
| `* 12 {+ 43 32}` | `(* 12 {(+ 43 32)})` |
| `amp: 0.5` | `[:amp 0.5]` |
| `"a": 1` | `["a" 1]` |
| `[amp: 0.5 pan: -1]` | `[[:amp 0.5] [:pan -1]]` |
| `once {s :crash} at: 4 gain: 0.5` | `(once {(s :crash)} [:at 4] [:gain 0.5])` |
| `let a 12` | `(let a 12)` |
| `a > fn1 12 > fn2 32 43` | `(fn2 (fn1 a 12) 32 43)` |
| `fn some-fn a:` / tab `fn1 a 12` / two tabs `> fn2 32 43` | `(fn some-fn a {(fn2 (fn1 a 12) 32 43)})` |
| `d :gain ? 1.0 > * 2` | `(* (#? (d :gain) 1.0) 2)` |
| `x b -> * x b` | `(-> (x b) (* x b))` |
| `map arr {x -> * x 2}` | `(map arr (-> (x) (* x 2)))` |
| `if {> a 10}:` / tab `print "big"` | `(if {(> a 10)} {(print "big")})` (the same as `if {> a 10} {print "big"}`) |
| `print -x` | `(print (#neg x))` |
| `"note {n} at beat {beat}"` | `(#interp "note " n " at beat " beat)` |
| `for i 0..8:` / tab `print i` | `(for i (.. 0 8) {(print i)})` |
| `let label if {> a 10} "big" "small"` | `(let label (if {(> a 10)} "big" "small"))` |

## Byte-Accurate Span Goldens (src/reader/tests/spans.rs)

- Source `let s "h\u{e9}llo" # c\r\nprint s\n` (the `é` is 2 bytes). Expected spans: `let` 0..3, `s` 4..5, the string 6..14,
  trivia comment 15..18, `print` 20..25, `s` 26..27. No span contains byte 18 (`\r`).
- Source `fn f a:\n\t* a 12\n`. Expected spans: `fn` 0..2, `f` 3..4, `a` 5..6, `*` 9..10, `a` 11..12, `12` 13..15.
- Source `#@ panel: gain\nlet g 1\n`. The trivia item has kind `Directive` and span 0..14.

## Negative and Recovery Tests (src/reader/tests/)

Each of the following yields its diagnostic, and the next line `print 1` still reads as `(print 1)`:
- `( + 1 2 )` gives `paren-form`.
- `let _tmp 1` gives `bad-identifier`.
- `let foo- 1` gives `bad-identifier`.
- `print _1` in a `FileId::new(1)` file gives `console-register-in-file`. The same line under `FileId::CONSOLE` reads `(print _1)`.

The following each produce their code:
- `indent-space`.
- `unexpected-indent`.
- `empty-block`.
- `continuation-after-block`.
- `unterminated-string`.
- `bad-escape`.
- `misplaced-colon`.
- `bad-pair` (`once gain:` followed by `> d1`).
- `misplaced-fallback` (`? 1`).
- `unclosed-group` (`print {+ 1`).
- `empty-group`.
- `bad-number` (`1st`, `1/0`).
- `stray-char` (`swap!`).
- `nesting-too-deep` (129 nested `{`).

A top-level `>` statement `> a 10` reads as `(> a 10)`, which is the head-position greater-than.

## Import Tests (src/reader/tests/import.rs)

- `import github.com/someone/vactrol-pads` reads `(#import "github.com/someone/vactrol-pads")` with prefix `pads`. The `as pd`
  form gives prefix `pd`, and `open` sets `open`.
- In a fresh env (`AliasEnv::new()`), the two-line document `import github.com/someone/vactrol-pads` then
  `note [:c3] > s pads.warm > d1` reads with no diagnostic. The `as pd` variant with `pd.warm` also reads clean.
- `s pads.warm` with no import gives `unbound-qualifier`. A bad path, such as uppercase text, a `..` segment or no `/`, gives `bad-import`.
  An import inside a block gives `import-not-top-level`.
- `prescan_imports` over a three-import document returns 3 decls without calling `read`.

## Spec Fixture Manifest (design 6.5.6)

- `tests/support/toml_subset.rs` parses the subset: `[[block]]` and `[[case]]` table arrays, `key = "basic"` with escapes
  `\" \\ \n \t`, `key = '''literal'''` (the first newline is trimmed), integers, booleans, string arrays, and `#` comments.
  An unsupported construct is a test failure with its line number.
- `manifest.toml` has 11 `[[block]]` entries. A block is found by its ordinal among fences whose info string is `vactrol` or
  `vact`, counted per document:
  - `lang-reference.md` ordinals 1-5: sections 0, 1, 2, 3, 4.
  - `design-music.md` ordinals 1-6: sections 1, 2, 3, 4, 5, 6.
- Each block entry has `reader = "clean"|"diagnostics"`, with `reader_diags = ["code@line"]` giving the exact
  multiset. It also has `expand = "unchecked"`; FE-EXPAND fills this in. It also has `eval = "unclassified"` and `note`.
  Blocks outside sections 1-3 may be `diagnostics`. For example, the `...` body in lang-reference section 4, and the
  continuation line without `>` in design-music section 6, are expected to give `stray-char` and `unexpected-indent`.
  Record whatever the reader actually produces, after checking that each diagnostic is correct under 6.5.4. If a
  diagnostic in sections 1-3 looks like a spec conflict, stop and report it; do not change the rules.
- `[[case]]` entries:
  - Every canonical expectation in the table above, with `verbatim = true` where the source appears verbatim in a spec document.
  - Pipe-continuation cases from design-music: section 1 lines 29-32 (the `every`/`whenmod` chain), section 3 lines 176-179
    (the three `d1` spellings), `slot :drums:` plus its block, and the `inst pluck ...:` header of section 2.
  - The negative examples.
  - The U2 pin (design 6.5.4): `id = "u2-gt-statement-level"`, `source = '> a 10'`, `read = '(> a 10)'`, `verbatim = false`.
    The pipe-continuation cases above pin the deeper-indented reading.
  - The two remaining authority questions (design 6.5.6), with `class = "authority-question"`:
    - `fn f a b:` / `* a 12` against `f 1 2  # => 24` (lang-reference section 1).
    - `let base 60` ... `upd base 62` (section 4).
  - The U4 inline-body case as an ordinary decided case (no `class`), `id = "u4-inline-fn-body"`,
    `source = 'fn kick-sound: :bd-haus'`, `verbatim = false` (lang-reference section 4 now writes this example in block
    form, lines 714-718, so the inline text no longer appears in the spec). The reader gives no diagnostic. Under 6.5.4
    `fn` reads as a flat item list and `kick-sound: :bd-haus` is a pair, so `read = '(fn [:kick-sound :bd-haus])'`.
    If the implemented 6.5.4 rules print something else, stop and report it; do not change the rules to fit.
  - For read now, only the read form is asserted. FE-EXPAND adds the expand forms and the U5 bare-variant binding-`if` case.
- `tests/spec_fixtures.rs` does the following:
  - Loads the manifest.
  - Reads each block (`FileId::new(1)`) and compares the diagnostic multiset.
  - For each case, checks `verbatim` containment and compares `read` (`print_all`) and `diags`. A case with `file = "console"` uses `FileId::CONSOLE`.
  - Checks that every block ordinal present in the documents has an entry. A missing entry fails the test.
- `src/reader/tests/no_panic.rs` reads every line-boundary prefix of all 11 blocks and asserts that nothing panics.

## Invariants

- 6.5.4 rules only. No new syntax. Every Decided item holds: identifier rule, tabs, `( )` as an error, `>` continuation,
  `key: value` as a pair, `_` as the wildcard, `#@` as trivia, and qualified names only after an import.
- No `.rs` file reaches 1000 lines. If `line.rs` or `lexer.rs` nears 800 lines, split a helper file inside `src/reader/` and record it.
- No panic on any input. No `unwrap`/`expect` on data derived from the input.
- No dependency is added.

## Edit Protocol

This is the same as the FE-VALUE Edit Protocol: a fresh read and a `shasum -a 256` hash before and after each shared-path edit, an intent
snapshot in the progress log, drift detection, and serial repair. `rustfmt` runs only on files this plan owns.

## Verification (foreground; logs under target/fe-logs/)

Run `mkdir -p target/fe-logs` first. Log evidence follows design 6.5.7 ("Verification evidence"):
- `LOG` for a cargo row is a new file `target/fe-logs/reader-<check>-s<S>-<n>.log` (`<S>` is the session number, `<n>`
  counts from 1 per check within the session). Never reuse or overwrite an existing file.
- Run every cargo row as `(set -o pipefail; CMD 2>&1 | tee LOG); echo "exit=$?" >> LOG`.
- The progress log cites, per cargo row, the counting log path (the last run after the final code change) and its
  `exit=` value. For V3 it also cites the run and passed counts, and the run count must be non-zero. A missing log, a
  log without `exit=`, or a truncated log fails the row. Non-cargo rows record their output inline.
In the tables, `\|` stands for a literal `|`.

| # | Command (`CMD`) | `<check>` | Evidence |
|---|---------|-----------|----------|
| V1 | `CARGO_TERM_QUIET=true cargo build` | `build` | `exit=0`, no warnings |
| V2 | `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` | `clippy` | `exit=0` |
| V3 | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run` | `nextest` | `exit=0`; a non-zero run count and 0 failed |
| V3f | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'binary(spec_fixtures)'` | `fixtures` | `exit=0`; the summary shows a non-zero run count and 0 failed. This proves the spec-fixture harness (`tests/spec_fixtures.rs`) ran; a run count of 0 fails the row |
| V4 | `find src tests -name '*.rs' -exec wc -l {} + \| sort -n \| tail -5` | - | every file is under 1000 lines |
| V5 | `grep -c '^\[\[block\]\]' tests/fixtures/spec/manifest.toml` | - | prints `11` |
| V6 | `git diff --stat -- Cargo.toml` | - | empty |

## Completion Criteria (they mirror vactrol-core.md TASK-002)

- [ ] All manifest fixtures read per their classification (console cases under `FileId::CONSOLE`); no blanket clean-parse claim
- [ ] Every negative example (`( )`, `_tmp`, `foo-`, `_1` in a file) yields a reader diagnostic, and the following lines still read
- [ ] `import`, `as` and `open` read correctly; the fresh-session `pads.warm` and `as pd` cases read; an unbound qualifier is an error; `prescan_imports` finds every import; `#@` lines land in trivia with positions
- [ ] Spans are byte-accurate (golden tests pass)
- [ ] V1-V6 and V3f pass (V3f: the `spec_fixtures` binary ran with a non-zero run count)

## Progress Log

### Plan revision: 2026-09-25 (session 167, plan author)
Design 6.5.6 now keeps two authority questions. The U4 inline-fn case is a decided case with `verbatim = false` and
the read form `(fn [:kick-sound :bd-haus])`. No other change.

### Plan revision: 2026-09-25 (session 169, plan author)
The verification table follows design 6.5.7: per-run logs `reader-<check>-s<S>-<n>.log`, `exit=` recorded inside each
log, and non-zero nextest run counts. With `NEXTEST_STATUS_LEVEL=fail`, nextest does not name passing tests, so the V3
requirement that "the log lists `spec_fixtures` tests" could not be met. It is replaced by the logged row V3f
(`cargo nextest run -E 'binary(spec_fixtures)'`, check `fixtures`, non-zero run count; Step 5 review finding). No
other change.

### Session: (implementer fills in)
**Tasks Completed**:
**Hashes / intent snapshots**:
**Verification evidence**:
**Diagnostic codes added to diag.rs (for FE-FINAL)**:
**Blockers**:

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-002)
- **Previous**: impl-plans/active/vactrol-frontend-value.md
- **Next**: impl-plans/active/vactrol-frontend-expander.md
