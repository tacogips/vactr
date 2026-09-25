# Vactrol Front End: Reader and Spec Fixture Manifest (FE-READER) Implementation Plan

**planId**: FE-READER (implements vactrol-core.md TASK-002)
**Status**: Completed (reconciled by FE-FINAL in session 175; stays in active/ until the user confirms archiving)
**Design Reference**: design-docs/specs/design-implementation.md sections 5.7, 6.1-6.3, 6.5.4, 6.5.6
**Created**: 2026-09-25
**Last Updated**: 2026-09-25 (session 172 checkpoint: review-only redispatch, review-verdict criterion)
**Issue**: https://github.com/tacogips/vactrol/issues/1
**dependsOn**: FE-VALUE at the source level, already satisfied in the working tree; dispatch dependsOn cleared by the session-172 amendment (wave 2; needs `Span`/`FileId`/`NodeId`, `Ratio64`, `Diagnostic`/`DiagCode`)

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
- `bad-pair`. The earlier example (`once gain:` followed by `> d1`) is not reachable under the 6.5.4 colon rules,
  because a trailing colon is the block opener (accepted review finding). Use an input that 6.5.4 actually routes to
  `bad-pair`, or, if no input reaches it, record that in the progress log and leave the code untested. Never change the
  lexer or colon rules to make a test pass.
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
| V3t | `CARGO_TERM_QUIET=true cargo test` | `cargotest` | exit 0 and a non-zero test count; run in addition to V3 because the workflow gate recognizes `cargo test` but not `cargo nextest run` as behavioral test evidence (2026-09-25) |
| V3f | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'binary(spec_fixtures)'` | `fixtures` | `exit=0`; the summary shows a non-zero run count and 0 failed. This proves the spec-fixture harness (`tests/spec_fixtures.rs`) ran; a run count of 0 fails the row |
| V4 | `find src tests -name '*.rs' -exec wc -l {} + \| sort -n \| tail -5` | - | every file is under 1000 lines |
| V5 | `grep -c '^\[\[block\]\]' tests/fixtures/spec/manifest.toml` | - | prints `11` |
| V6 | `git diff --stat -- Cargo.toml` | - | empty |

## Completion Criteria (they mirror vactrol-core.md TASK-002)

- [x] All manifest fixtures read per their classification (console cases under `FileId::CONSOLE`); no blanket clean-parse claim
- [x] Every negative example (`( )`, `_tmp`, `foo-`, `_1` in a file) yields a reader diagnostic, and the following lines still read
- [x] `import`, `as` and `open` read correctly; the fresh-session `pads.warm` and `as pd` cases read; an unbound qualifier is an error; `prescan_imports` finds every import; `#@` lines land in trivia with positions
- [x] Spans are byte-accurate (golden tests pass)
- [x] V1-V6 and V3f pass (V3f: the `spec_fixtures` binary ran with a non-zero run count)
- [x] A schema-valid adversarial review verdict (a `findings` array, empty if none) exists for the tree in `tmp/fe-frontend-20260925-s165/FE-READER/attempt-1/final-hashes.txt`, and it covers RECON2-2 (`empty-pipe`) and RECON2-3 (the `:7` spec conflict). Evidence: `tmp/fe-frontend-20260925-s165/FE-READER/attempt-2/`

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

### Plan revision: 2026-09-25 (session 170, plan author)
The accepted low review finding on the unreachable `bad-pair` example (dispatch manifest `acceptedReviewFindings`) is
now in the negative-test list itself. No other change.

### Session: 2026-09-25 (session 170, FE-READER implementer)
**Tasks Completed**: TASK-002 reader implemented per design 6.5.4 and the fixture manifest per 6.5.6.
- `src/reader/{node,lexer,layout,line,import,sexpr}.rs` and `mod.rs` (`read`, `ReadResult`, re-exports). `span.rs` is unchanged.
- Unit tests in `src/reader/tests/` (31): the canonical table, span goldens, negative and recovery cases, every reader code, imports and trivia, the nesting cap, and the no-panic test (line-boundary and char-boundary prefixes of all 11 blocks, plus hostile inputs).
- `tests/spec_fixtures.rs`, `tests/support/{mod,toml_subset}.rs` and `tests/fixtures/spec/manifest.toml`: 11 blocks and 50 cases, 7 runner tests.
- All 17 canonical expectations print exactly as the table says.
**Design differences and decisions (6.5.4 wins; recorded for review)**:
- SPEC CONFLICT (reported; rules unchanged). design-music sections 2 and 3 write `[:g :7]` (lines 114 and 242). `:7` is not a keyword, because the Decided identifier rule forbids a leading digit. Under the 6.5.4 colon classes it reads as `misplaced-colon`. Blocks music #2 and #3 are therefore `diagnostics` (`misplaced-colon@40`, `misplaced-colon@104`). Case `music-chord-seven-conflict` pins this. The language author needs to decide: rename the chord quality (for example `:dom7`), or amend the identifier or keyword rule. Every other construct in sections 1-3 of both documents reads clean.
- Blocks outside sections 1-3 behave as the plan expected:
  - lang #5 gives `stray-char@92`: the `...` body is `..` followed by a stray `.`.
  - music #6 gives `unexpected-indent@3`: a continuation line without `>`.
- A deeper line without `>` is `unexpected-indent`. It and the lines under it belong to the statement it is indented under, and that statement becomes the `Error` node, because the statement is the unit of recovery. A later `>` line still continues the statement, as 6.5.4 says. A deeper line with no statement before it in the body becomes an `Error` node of its own.
- `bad-pair` is reachable under 6.5.4: `[amp: ]` and `once gain: > d1`, where the colon is followed by a space and the operand ends. No lexer or colon rule was changed.
- `( + 1 2 )` gives one `paren-form`: every `(` reports, and a `)` reports only when it closes nothing.
- A run of stray characters is one `stray-char`.
- An unmatched `}` or `]` is `stray-char`.
- A failed nested string reports once; enclosing strings add nothing.
- `_` is a path character in `import` (6.5.4 `[a-z0-9._-]`), so `some_one` is a valid segment.
- The `prescan_imports` spans carry `FileId::CONSOLE`, because the plan signature has no file argument. The byte offsets are exact.
- Stack safety beyond the 128 cap (never panic):
  - Arrow chains and prefix chains (`& -x`, `a: b: 1`) are built in loops.
  - Pipes, fallbacks and arrows count toward a total tree-depth backstop of 1024, reported as `nesting-too-deep`.
  - Measured stack for 128 nested groups: about 1.1 MiB in a debug build and 0.25 MiB in release, within the 2 MiB test threads and the 1 MiB wasm stack.
- IfChain grouping (`group_if_chains`) lives in `layout.rs`, as this plan assigns. That also keeps `line.rs` at 741 lines, below the 800-line split trigger. No helper file was added.
**Hashes / intent snapshots**: `tmp/fe-frontend-20260925-s165/FE-READER/attempt-1/`
- `pre-edit-hashes.txt`: `src/reader/mod.rs` ae28f82c, `src/types/diag.rs` da994cbe, `src/lib.rs` b5d8b093. `src/lib.rs` was not edited.
- `intent.md`: every shared-path hunk.
- `post-edit-hashes.txt`: `diag.rs` bf9b818f; `mod.rs` 40297ff8, then ff145791, then 091a2f07.
- `final-hashes.txt`: 20 files.
- `gen_manifest.py`: the manifest generator, which keeps the tabs literal.
**Verification evidence** (the last run of each check after the final code change; all under `target/fe-logs/`):
- V1 `reader-build-s170-2.log`: exit=0.
- V2 `reader-clippy-s170-2.log`: exit=0.
- V3 `reader-nextest-s170-2.log`: exit=0; 99 tests run, 99 passed, 0 skipped.
- V3t `reader-cargotest-s170-2.log`: exit=0; lib 92 passed and spec_fixtures 7 passed, 0 failed.
- V3f `reader-fixtures-s170-2.log`: exit=0; 7 tests run, 7 passed.
- Extra, because `diag.rs` is an FE-VALUE file: `reader-wasm32-s170-2.log` exit=0 and `reader-wasm32-hostwasm-s170-2.log` exit=0.
- Superseded runs `*-s170-1.log` stay on disk. They also all passed.
- V4: largest files are `line.rs` 741, `lexer.rs` 690 and `layout.rs` 343 lines. Every file is under 1000.
- V5: prints `11`.
- V6: `git diff --stat -- Cargo.toml` shows only FE-VALUE's uncommitted `[features]` table (6 insertions). The Cargo.toml sha256 f8e41f7e equals the FE-VALUE post-edit hash, so FE-READER did not change Cargo.toml.
- `grep -rnE 'std::(thread|fs|time|net|process)' src/` prints `none`.
- `rustfmt --check` on the owned files: exit 0.
**Diagnostic codes added to diag.rs (for FE-FINAL)**:
- `EmptyPipe => "empty-pipe"`, for a pipe `>` with no call after it (`print 1 >`). 6.5.4 defines no code for an empty trailing segment, and passing `>` as a value is not allowed.
- The FE-VALUE count test is now 38 (27 reader and 11 expander codes). FE-FINAL records `empty-pipe` in 6.5.4 alongside `misplaced-arrow`.
**Blockers**: none. The `:7` spec conflict above needs a language-author decision; it is not an implementation blocker, because the rules were kept as decided.

### Plan revision: 2026-09-25 (session 172, plan checkpoint)
Session 171 dispatched only FE-VALUE, so FE-READER still has no review verdict (integration findings RECON3-1 and
session-170 finding-1). The dispatch dependsOn on FE-VALUE is cleared, so FE-READER is dispatchable now. This rerun is
review-only: re-check `final-hashes.txt` (20/20 OK), re-verify with s172 logs, then run step7 into `attempt-2/`. The
review-verdict criterion above mirrors the dispatch manifest acceptance criterion. No requirement or code change.

### Session: 2026-09-25 (session 172, FE-READER step6 re-verification)
**Tasks Completed**: review-only redispatch (RECON3-1). No source file was edited. The tree is unchanged since session 170:
`reader-hashes-s172-1.log` shows 20/20 OK, exit=0. Cargo.toml sha256 is still f8e41f7e (FE-VALUE's `[features]` table only).
**Verification evidence** (all under `target/fe-logs/`):
- V1 `reader-build-s172-1.log`: exit=0.
- V2 `reader-clippy-s172-1.log`: exit=0.
- V3 `reader-nextest-s172-1.log`: exit=0; 99 tests run, 99 passed, 0 skipped.
- V3t `reader-cargotest-s172-1.log`: exit=0; lib 92 passed and spec_fixtures 7 passed, 0 failed.
- V3f `reader-fixtures-s172-1.log`: exit=0; 7 tests run, 7 passed.
- Extra: `reader-wasm32-s172-1.log` exit=0.
- `reader-rustfmt-s172-2.log`: `rustfmt --check` on the 19 owned `.rs` files, exit=0. The `-s172-1` run failed only because
  zsh did not split the file list; it is superseded.
- V4: the largest files are `line.rs` 741, `lexer.rs` 690 and `layout.rs` 343 lines. V5 prints `11`. V6 shows only FE-VALUE's
  6-line `[features]` table. The `std::(thread|fs|time|net|process)` grep prints `none`.
**Pending (step7)**: the adversarial review verdict in `attempt-2/` must cover RECON2-2 (`empty-pipe`) and RECON2-3 (the
`:7` conflict). The last completion criterion stays unchecked until then.

### Session: 2026-09-25 (session 172, reconcile-implementations: step7 verdict persisted)
**Tasks Completed**: The step7 adversarial review-only verdict (`step7-adversarial-review-attempt-1-exec-210`, accepted
01:03:12Z) is `accepted=true`, `needs_revision=false`, `findings=[]`. It was copied verbatim from the Riela session record to
`tmp/fe-frontend-20260925-s165/FE-READER/attempt-2/step7-review-verdict.json`. The review re-ran the `final-hashes.txt` check
(20/20 OK) and covers RECON2-2 (`empty-pipe` matches design 6.5.4 step 2; the `line.rs:169` branch cannot be reached and is a
harmless guard) and RECON2-3 (`[:g :7]` reads as misplaced-colon and is pinned for the language author). The review-verdict
criterion is now checked. No source was edited. Combined-tree checks: `tmp/fe-frontend-20260925-s165/reconcile-implementations/attempt-4/`.
**Residual (not in this plan)**: FE-FINAL records `empty-pipe` and `misplaced-arrow` in design 6.5.4. The `:7` decision belongs to the language author.
Plan acceptance still belongs to the integration review.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-002)
- **Previous**: impl-plans/active/vactrol-frontend-value.md
- **Next**: impl-plans/active/vactrol-frontend-expander.md
