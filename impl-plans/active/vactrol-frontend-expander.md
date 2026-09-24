# Vactrol Front End: Expander and Kernel Forms (FE-EXPAND) Implementation Plan

**planId**: FE-EXPAND (implements vactrol-core.md TASK-003)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md sections 6.4, 6.5.5, 6.5.6
**Created**: 2026-09-25
**Last Updated**: 2026-09-25 (session 169 revision: design 6.5.7 verification evidence rule)
**Issue**: https://github.com/tacogips/vactrol/issues/1
**dependsOn**: FE-READER (wave 3; needs `Node`, `NodeKind`, `Atom`, `ReadResult::next_node_id`, `sexpr::print`, and the manifest runner)

---

## Intent and Context

This plan implements the compile-time expander. It is a pure `Node -> Node` pass that removes all sugar
(`if`/`elif`/`else`, binding `if`, `for`, `?`, `-x`, string interpolation) and validates that the output
contains only the Decided kernel forms `match fn let var upd -> {} enum`, plus calls. It then extends
the fixture manifest with expand results. Design 6.5.5 is normative. Its desugaring table must be reproduced
exactly, because the plan criterion "desugarings match the `# ~`/`# ==` annotations" is judged against it.

## Non-Goals

- No pattern compilation, no variant resolution, no checker diagnostics (unknown names, the redundant-clause lint), and no `concat` native.
- No argument reordering and no keyword-argument resolution. Pairs and splats pass through unchanged (6.5.5).
- No special handling of `while`, `loop`, `break`, `when`, `unless` or `each`. They are ordinary unknown names for TASK-004.
- `struct` and `inst` are not reserved (design Q3 is still open).

## writePaths (exclusive)

- `src/expand/mod.rs`: `expand`, `ExpandCx`, re-exports, and `#[cfg(test)] mod tests;`.
- `src/expand/expander.rs`: traversal, and the classification into expression, pattern and header positions.
- `src/expand/sugar.rs`: rewrites.
- `src/expand/kernel.rs`: shape validation and `pub fn is_kernel(&Node) -> bool`.
- `src/expand/tests/mod.rs`, `if_match.rs`, `for_map.rs`, `small_sugar.rs`, `kernel.rs`, `diagnostics.rs`.

## sharedPaths (serial edits; use the Edit Protocol)

- `src/lib.rs`: add one line, `pub mod expand;`.
- `src/types/diag.rs`: change it only if an expander code is missing. Add the code and record it in the progress log for FE-FINAL.
- `tests/fixtures/spec/manifest.toml`: set each block's `expand` field to `"clean"` or `"diagnostics"`, with `expand_diags`.
  Add `expand` strings to the cases and add the new U5 case listed below.
- `tests/spec_fixtures.rs`: extend it to expand every form of every block and case. A form that contains an `Error` node is skipped,
  and the test asserts that it returns `read-error-present`. The runner compares the diagnostic multiset and `print_all` output, and
  asserts `is_kernel` on every successful output.

## File-Level Changes (signatures only)

- `pub struct ExpandCx`, with a private next-id counter, and `ExpandCx::new(first_free: NodeId) -> Self`. Synthesized nodes take
  fresh ids from it and carry the span of the sugar node they replace.
- `pub fn expand(n: &Node, cx: &mut ExpandCx) -> Result<Node, Diagnostic>`. The first diagnostic ends expansion of the form.
  It returns `read-error-present` when `n.contains_error()`.
- Positions (6.5.5):
  - Expression positions are expanded. They are call heads and arguments, list items, pair values, splat operands,
    block statements, arrow bodies, and a match guard (the single item after a top-level `if` in a match clause lhs).
  - Pattern and header positions are not expanded. They are arrow lhs items, `let`/`var`/`upd`/`for` targets, `fn`
    header items and `enum` bodies. Any `Neg`, `Fallback`, `Interp` or `IfChain` found there is `sugar-in-pattern`.
- Rewrites (`FN` stands for the three atoms `false`, `|` (the `Op::Bar` atom), `nil`):
  - `(if C T E)` becomes `(match C {(-> (FN) E) (-> (_) T)})`, and `(if C T)` does the same with `E = nil`.
  - A binding `if` is an `Arrow` whose lhs is `[if, S, P..]`:
    - When `P` is one `Sym` or a `Wildcard`, it becomes `(match S {(-> (FN) E) (-> (P) T)})`.
    - Otherwise it becomes `(match S {(-> (P..) T) (-> (_) E)})`.
    - A top-level `if` inside `P..` is `if-guard`.
  - An `IfChain` nests: the else value of each link is the expansion of the rest of the chain, and the last `else` supplies
    its single argument (a Block or an expression). When there is no `else`, the value is `nil`.
  - `(for P S B)` becomes `(match (map S (-> (P) B)) {(-> (_) nil)})`, where `map` is `Atom::Builtin("map")`.
  - `Fallback(X, D)` becomes `(or X D)`, `Neg(X)` becomes `(neg X)`, and `Interp(parts)` becomes `(concat parts..)`,
    with empty text pieces dropped. The heads `or`, `neg` and `concat` are `Atom::Builtin`.
  - The kernel heads `match`, `->` and the rest are the plain `Sym` names. The pattern atoms `false`, `nil` and `_` are `Atom::Bool(false)`,
    `Atom::Nil` and `Atom::Wildcard`.
- Kernel validation (`kernel.rs`), with codes exactly as in design 6.5.5:
  - `match` must have the shape `(match SUBJ {Arrow+})` with every lhs non-empty, else `malformed-match`.
  - `fn` must have the shape `(fn NAME HEADER* Block)` with `NAME` a `Sym`, else `malformed-fn`.
  - `let` and `var` must have 2 arguments, with a `Sym`, a pair whose key is a keyword, or a `List` target. `upd` must be `(upd Sym EXPR)`. Otherwise `malformed-binding`.
  - `enum` must have the shape `(enum Sym Block)` with each line a `Sym` or a `Sym`-headed call, else `malformed-enum`.
  - A reserved word used as a binding name, fn name, parameter or target is `reserved-word`. The reserved words are `match fn let var upd enum if elif else for import`, plus the atoms nil, true and false.
  - A stray `elif` or `else` is `else-without-if`. A wrong `if` arity is `malformed-if`. A `for` without exactly a pattern, a source and a body is `malformed-for`.
  - A `Splat` that is neither a call argument nor a list item is `misplaced-splat`.
- `is_kernel` returns true only when the tree has no `Neg`, `Fallback`, `Interp`, `IfChain` or `Error` node, no call headed by
  `if`/`elif`/`else`/`for`, `Builtin` atoms only in call-head position, `Import` only at the root, and every kernel head
  in its shape.

## Canonical Expectations (the `print` of the expand output must match exactly)

| Source | Expanded |
|--------|----------|
| `if {> a 10} "big" "small"` | `(match {(> a 10)} {(-> (false \| nil) "small") (-> (_) "big")})` |
| `let maybe if {> a 100} "huge"` | `(let maybe (match {(> a 100)} {(-> (false \| nil) nil) (-> (_) "huge")}))` |
| `if {d :gain} g -> once {s :bd} gain: g` | `(match {(d :gain)} {(-> (false \| nil) nil) (-> (g) (once {(s :bd)} [:gain g]))})` |
| lang-reference section 3 block `if {d :gain} g ->:` / `else:` | `(match {(d :gain)} {(-> (false \| nil) {(once {(s :bd)})}) (-> (g) {(once {(s :bd)} [:gain g])})})` |
| lang-reference section 3 `if {parse s} ok v ->:` / `else:` | `(match {(parse s)} {(-> (ok v) {(print v)}) (-> (_) {(print "parse failed")})})` |
| `if`/`elif`/`else` block (lang-reference section 3, lines 516-521) | `(match {(> a 10)} {(-> (false \| nil) (match {(> a 5)} {(-> (false \| nil) {(print "small")}) (-> (_) {(print "medium")})})) (-> (_) {(print "big")})})` |
| `for x arr:` / tab `print x` | `(match (map arr (-> (x) {(print x)})) {(-> (_) nil)})` |
| `d :gain ? 1.0 > * 2` | `(* (or (d :gain) 1.0) 2)` |
| `print -x` | `(print (neg x))` |
| `"note {n} at beat {beat}"` | `(concat "note " n " at beat " beat)` |
| `once {s :crash} & p` | `(once {(s :crash)} (& p))` |
| `let v voice note: 60 pan: -1` | `(let v (voice [:note 60] [:pan -1]))` |

(In the table, `\|` stands for a literal `|`.)

## Required Unit Tests (src/expand/tests/)

- `if_match.rs`: the first six rows above; `elif` binding form; `if` inside `let`; `if c X` with no else, which gives nil.
- `for_map.rs`: `for x arr:`, `for [k v] d:` (the list pattern is kept as is) and `for [i x] {enumerate arr}:`.
- `small_sugar.rs`: `?`, including its precedence with `>`; `-x`; `-{f x}`; interpolation with an empty text piece; the splat passing through.
- `kernel.rs`: `is_kernel` is true on every output above and false on a tree that still holds a `Neg`; kernel shapes are valid; a guard
  in a match clause (`x if {> x 100} -> "big"`) expands the guard only.
- `diagnostics.rs`: each expander code (`malformed-if`, `else-without-if`, `if-guard`, `malformed-for`, `malformed-match`,
  `malformed-fn`, `malformed-binding`, `malformed-enum`, `sugar-in-pattern`, `reserved-word`, `misplaced-splat`,
  `read-error-present`) is returned, and its `span` equals the span of the originating source node (the origin span).
- Synthesized node ids are all at least `first_free` and unique within one output.

## Manifest Additions (tests/fixtures/spec/manifest.toml)

- Set `expand` for all 11 blocks. Sections 1-3 of both documents must be `clean` unless a form is a documented authority
  question. The lang-reference section 4 block now writes `fn kick-sound:` in block form (lines 714-718), so those forms
  must expand clean (design 6.5.5); any section 4 `expand_diags` come only from forms that already had reader errors.
- Add `expand` to every existing case where the table above or FE-READER defines one.
- Decided cases (no `class`; U4 and U5 are answered, design 6.5.6):
  - `u4-inline-fn-body` (added by FE-READER, `verbatim = false`, read clean): change `diags` from `[]` to
    `["malformed-fn"]` and add no `expand` string. Rule for every case: `diags` is the multiset of reader codes plus
    expander codes. When FE-EXPAND extends `tests/spec_fixtures.rs` to expand cases, it appends the expander diagnostic
    (if any) to the reader diagnostics before comparing; a case with an expander diagnostic has no `expand` string.
  - New case `u5-bare-variant-binding-if`, `source = 'if x none -> 1'`, `verbatim = false`, expanding to
    `(match x {(-> (false | nil) nil) (-> (none) 1)})`, with `note` stating that TASK-004 (checker) must diagnose a
    bare field-less variant used as the binding pattern of `if`. The expander implements no such diagnostic.
- Authority-question cases (`class = "authority-question"`): the `fn f a b:` / `* a 12` and `let base`/`upd base` cases get
  their expand forms. Evaluation stays pending.

## Invariants

- The output is kernel-only (`is_kernel`) for every successfully expanded fixture form.
- Every Decided item holds: `if`/`elif` are sugar over `match` (the only place truthiness is defined), `for` is sugar over `map`, and there is no loop form.
- Expansion never panics. Recursion depth is bounded by the reader's nesting cap of 128 plus a constant.
- No `.rs` file reaches 1000 lines. No dependency is added.

## Edit Protocol

This is the same as FE-VALUE: a fresh read and a `shasum -a 256` hash before and after each shared-path edit (`src/lib.rs`, the manifest and
`tests/spec_fixtures.rs`), an intent snapshot in the progress log, and a stop-and-reapply when drift is detected.

## Verification (foreground; logs under target/fe-logs/)

Run `mkdir -p target/fe-logs` first. Log evidence follows design 6.5.7 ("Verification evidence"):
- `LOG` for a cargo row is a new file `target/fe-logs/expand-<check>-s<S>-<n>.log` (`<S>` is the session number, `<n>`
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
| V3f | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'binary(spec_fixtures)'` | `fixtures` | `exit=0`; a non-zero run count and 0 failed (proves the extended fixture harness ran) |
| V4 | `find src tests -name '*.rs' -exec wc -l {} + \| sort -n \| tail -5` | - | every file is under 1000 lines |
| V5 | `grep -c '^expand = ' tests/fixtures/spec/manifest.toml` | - | at least 11 (every block has an expand classification) |
| V6 | `grep -n 'expand = "unchecked"' tests/fixtures/spec/manifest.toml \|\| echo none` | - | prints `none` |

## Completion Criteria (they mirror vactrol-core.md TASK-003)

- [ ] The desugarings match the `# ~ (...)` and `# ==` annotations in lang-reference.md (canonical table above)
- [ ] The output tree contains kernel forms only (asserted structurally through `is_kernel`)
- [ ] Malformed forms produce the listed diagnostics with their origin span
- [ ] V1-V6 and V3f pass

## Progress Log

### Plan revision: 2026-09-25 (session 167, plan author)
U4 and U5 are answered, so their cases are decided cases, not authority questions. The section 4 block is expected to
expand clean for `fn kick-sound:`. The U5 case notes the TASK-004 checker obligation. No other change.

### Plan revision: 2026-09-25 (session 169, plan author)
The verification table follows design 6.5.7: per-run logs `expand-<check>-s<S>-<n>.log`, `exit=` recorded inside each
log, and non-zero nextest run counts. V3f (`cargo nextest run -E 'binary(spec_fixtures)'`, check `fixtures`) is added so
the log proves the extended fixture harness ran, because `NEXTEST_STATUS_LEVEL=fail` does not name passing tests. No other change.

### Session: (implementer fills in)
**Tasks Completed**:
**Hashes / intent snapshots**:
**Verification evidence**:
**Blockers**:

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-003)
- **Previous**: impl-plans/active/vactrol-frontend-reader.md
- **Next**: impl-plans/active/vactrol-frontend-finalize.md
