# CMP-10: Completion Engine Core (`src/complete/`)

**Status**: Ready
**Plan ID**: CMP-10 (wave 1; parallel with CMP-15, FST-50, EDS-10, EDS-11, EDS-12)
**Design Reference**: `design-docs/specs/design-completion.md` sections 3.1-3.8, 7.1
**Dispatch**: `impl-plans/active/cmp-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

The user wants suggestions to appear while they type. Today, completion
exists only in `src/lsp/analysis.rs:337` `Analyzer::complete`, and it
filters with `starts_with`. This plan builds the ONE shared engine as a
pure, wasm32-clean module. CMP-20 (LSP), CMP-21 (wasm export) and the
editor consume it later.

Key repository facts:

- `reader::read` turns an erroring statement into a childless `Error`
  node (`src/reader/line.rs:295-314`), so the engine must NOT use `read`
  nodes.
- It uses the layout skeleton instead: `crate::reader::layout::{split_lines, statements}`
  gives `Line`, `Stmt` and `BlockSkel` (`src/reader/layout.rs:15-60`).
  These are `pub(crate)` and keep their tokens even for erroring
  statements. Tokens are `crate::reader::lexer::{Token, Tok}`.

## Non-goals

- No change to any file under `src/reader/`, `src/lsp/`, `src/host/` or
  `src/fmt/`, or to any editor file.
- No signature help, no snippets, no `open`-import unqualified names.
- No new crate. `serde` and `serde_json` are already core dependencies.

## Dependencies

- **dependsOn**: none
- **Blocks**: CMP-20, CMP-21, CMP-40

## writePaths

- `src/complete/mod.rs`
- `src/complete/context.rs`
- `src/complete/scope.rs`
- `src/complete/sources.rs`
- `src/complete/rank.rs`
- `src/complete/tests/mod.rs`
- `src/complete/tests/context.rs`
- `src/complete/tests/scope.rs`
- `src/complete/tests/rank.rs`
- `src/complete/tests/robust.rs`
- `src/lib.rs`: add exactly one line, `pub mod complete;`, in alphabetical
  order after `pub mod compile;`. Also add one line to the module doc
  list. Nothing else changes.
- `impl-plans/active/cmp-10-complete-core.md` (Progress Log only)

## sharedPaths (read only)

`src/reader/layout.rs`, `src/reader/lexer.rs`, `src/reader/import.rs`,
`src/reader/mod.rs`, `src/types/natives.rs`, `src/types/manifest.rs`,
`src/dsp/meta.rs`, `src/dsp/ugen/catalog.rs`, `src/fmt/mod.rs` (the
pattern for `format_bytes`), and `src/fmt/tests/mutation.rs` (the
xorshift and the mutation tokens).

## Public contract (pinned; CMP-20 and CMP-21 depend on it exactly)

```rust
pub const DEFAULT_LIMIT: usize = 100;
pub const MAX_LIMIT: usize = 500;
pub const STATUS_OK: u32 = 0;
pub const STATUS_NOT_UTF8: u32 = 2;
pub const STATUS_BAD_CURSOR: u32 = 3;
pub enum ContextKind { None, Keyword, Qualified, PipeTarget, Head, PairKey, Argument }
pub enum CandidateKind { Local, Function, Value, Variable, Type, Keyword, Control, Key, Module, Qualified }
pub struct Candidate { pub label: String, pub kind: CandidateKind, pub detail: String, pub insert: String }
pub struct Completion { pub context: ContextKind, pub from: usize, pub to: usize, pub items: Vec<Candidate>, pub incomplete: bool }
pub struct PackageNames { pub prefix: String, pub path: String, pub names: Vec<String> }
pub struct DocName { pub label: String, pub kind: CandidateKind, pub detail: String }
pub struct Snapshot { pub manifest: HostManifest, pub packages: Vec<PackageNames>, pub document: Vec<DocName> }
impl Snapshot { pub fn builtin() -> Snapshot }
pub fn complete(text: &str, cursor: usize, snap: &Snapshot, limit: usize) -> Completion
pub fn complete_bytes(input: &[u8], cursor: u32, limit: u32) -> (u32, Vec<u8>)
impl Completion { pub fn to_json(&self) -> String }
```

- Derive `Clone, Debug, PartialEq, Eq` where possible. `Copy` goes on
  the two enums.
- JSON (design 5.2):
  - the object is
    `{"v":1,"context":..,"from":..,"to":..,"incomplete":..,"items":[{"label","kind","detail","insert"}]}`;
  - `context` strings are `none keyword qualified pipe head pair-key argument`;
  - `kind` strings are the lowercase variant names.

  Use serde `rename` or `rename_all`, or hand-built `serde_json::json!`.
  Either is fine as long as a test asserts the exact field names and
  strings.
- `complete_bytes` returns:
  - for non-UTF-8 input: `(STATUS_NOT_UTF8, vec![])`;
  - for `cursor as usize > len`: `(STATUS_BAD_CURSOR, vec![])`;
  - otherwise `(STATUS_OK, to_json().into_bytes())`.

  A `limit` of 0 means `DEFAULT_LIMIT`. It caches `Snapshot::builtin()` in
  a `thread_local!`, as `src/host/wasm/fmt_abi.rs` does for `FMT_OUT`.
  Do not use `static mut`.
- `Snapshot::builtin()` holds `HostManifest::spec_default()` and empty
  `packages` and `document`.

## Implementation Key Points

1. **Cursor normalization.** Clamp `cursor` to `text.len()`, then step it
   back until `text.is_char_boundary(cursor)`. Clamp `limit` to
   `1..=MAX_LIMIT`. `to` is always the normalized cursor.
2. **Line scan (`context.rs`).** Scan `text[line_start..cursor]` byte by
   byte. Track:
   - whether the scan is inside a string (`"` toggles it, and `\` skips
     the next byte);
   - the interpolation depth inside a string (`{` and `}`);
   - a `#` outside any string, which means a comment.

   In string text, or in a comment, the result is `ContextKind::None`
   with no items. Inside an interpolation the position is code.
3. **Word.** Walk back from the cursor over identifier bytes
   `[A-Za-z0-9-]`, then take:
   - an optional single `.` preceded by identifier bytes, for
     `prefix.rest`;
   - then an optional leading `:`.

   `from` is the start of the word. Use `lexer::is_identifier` (at
   `src/reader/lexer.rs:148`) for the prefix check.
4. **Contexts, in design 3.3 table order (first match wins).** Use the
   cursor statement's tokens whose span ends at or before `from`. Get
   them from the skeleton `Stmt` whose span contains the cursor line.
   When the cursor line is not in the skeleton (blank or tab-only), use
   `lex_line` on the cursor line, which is `pub(crate)`.
   - `>`: a line-initial `>` is a pipe. A `>` right after `{` is
     greater-than, so the position is an `Argument`.
   - Nothing after a pair colon: `name:|` with no space gives `None`, and
     so does Ctrl-Space. The engine has no manual mode (design 3.3, last
     bullet).
   - Definition-name positions: the identifier right after
     `let var fn inst struct enum bus look`, and header parameter names
     of `fn`/`inst` before the block colon. They give `None`. Default
     values after a header pair colon are not definition names, and
     `:`-words never are.
   - PairKey: the head's key set is non-empty AND (an earlier pair
     appears in the segment OR the positional count has reached the known
     max arity). A piped subject counts as the first positional argument.
5. **Scope (`scope.rs`), design 3.4 steps 1-7.** Pitfalls:
   - Choose the enclosing blocks by the cursor line's LEADING TAB COUNT
     `L`. In each body, take the last statement starting at or before the
     cursor, and descend only when `L >= inner level`.
   - Header pairs: bind the key, then skip everything up to the next
     `Ident` + `PairColon` or up to the block colon. This covers
     `cutoff: float = 1200`.
   - Block locals are visible only when they start BEFORE the cursor's
     statement.
   - Binders exclude pair keys, the contents of `{}` groups and the
     reserved words `if elif else match for fn let var upd`.
   - The innermost frame first. Deduplicate by label, and the first
     occurrence wins.
   - Bound the recursion at 128 (`reader::MAX_NESTING`), or write the
     walk iteratively.
6. **Sources (`sources.rs`), design 3.5:**
   - `NativeTable::global().iter()`: kind `Function` or `Value` from
     `sig.kind`, detail `sig.ty.first()` or `"prelude"`.
   - Keyword sets from `snap.manifest`, where `KeySet::Of(keys)` is the
     only enumerable variant. Synths also get
     `dsp::ugen::catalog::TEMPLATE_NAMES` and the document's `inst`
     names. Labels are `:name` with detail `sound`, `synth` or
     `control`.
   - Key sets, from `NativeSig::keywords`, the document `fn`/`inst`
     header keys, and `dsp::meta::decl_for(head).params[].name`. Label
     `name:`, insert `name: `, kind `Key`.
   - Seed controls: the statement starts `s` or `sound` followed by
     `Tok::Keyword(name)`. The controls are `decl_for(name)` params, or
     the header keys of a document `inst name`. Kind `Control`, label and
     insert are the bare name.
   - Prefixes from `reader::prescan_imports(text)`: kind `Module`, detail
     is the path. Qualified names from `snap.packages`.
   - `snap.document` merges into the document tier, and its detail wins.
7. **Ranking (`rank.rs`), design 3.6:**
   - Matching is ASCII case-insensitive. The classes are prefix, subword
     (a segment start after `-` or `.`, or initials) and fuzzy (a
     subsequence whose first matched char is at a segment start).
   - Deduplicate first, keeping the lowest `(tier, depth)`.
   - The sort key is `(class, tier, depth, suborder, label bytes)`.
   - Truncate to `limit` and set `incomplete`.
   - In a `Keyword` context, match against the label without its `:`. In
     a `Qualified` context, match against the part after the `.`.
8. **No panics.**
   - Never index with `[]` on a computed offset. Use `get(..)`.
   - Never `unwrap` or `expect` outside tests.
   - Tests run with `debug_assertions`. Arithmetic uses `saturating_*`
     where it could overflow.
9. **File sizes.** Every file stays under 1000 lines. Split test files
   further if they approach that.

Patterns to imitate:

- `src/fmt/mod.rs` for `format_bytes`, the status consts and the module
  doc style;
- `src/fmt/tests/mutation.rs` for `XorShift64` and `mutate`. Copy them;
  do not import them, because that module is private to `fmt`;
- `src/fmt/tests/corpus.rs` `committed_sources` for enumerating the
  committed `.vact` files. Copy its approach.

## Tests (`src/complete/tests/`; the cursor is the `|` position, removed before calling)

- context.rs:
  - `|` at the start of an empty doc -> `Head`.
  - `{m|` -> `Head`.
  - `x -> pr|` -> `Head`.
  - `s :bd > ga|` -> `PipeTarget`.
  - `x\n\t> ga|` -> `PipeTarget`.
  - `{> a|` -> `Argument`.
  - `foo 1 ba|` -> `Argument`.
  - `s :a|` -> `Keyword`, with `from` at the `:`.
  - `drums.k|` with `import github.com/x/drums` -> `Qualified`.
  - PairKey through meta: pick a head `h` whose
    `dsp::meta::decl_for(h)` has at least two params `p1` and `p2` (read
    them in the test, do not hardcode them). `h 1 p1: 2 <first letter of p2>|`
    -> `PairKey`, and `p2:` ranks in tier 0 before any local.
  - PairKey at a native arity limit: pick, in the test, a native `n` from
    `NativeTable::global().iter()` whose `sig.keywords` is non-empty and
    whose `sig.max_args` is `Some(k)` with `k >= 1` (read them in the
    test, do not hardcode). `n` followed by `k` positional number arguments and then
    the first letter of one of its keywords, with the cursor at the end
    -> `PairKey`, and that keyword's `name:` is in the items. With fewer
    than `k` positional arguments and no earlier pair -> `Argument`.
  - `"ab|c"` -> `None`.
  - `# co|` -> `None`.
  - `"{fo|}"` -> not `None`.
  - `12|` -> `None`.
  - `let na|` -> `None`.
  - `fn f pa|` -> `None`.
  - `gain:|` -> `None`.
  - `gain: |` -> `Argument`.
- scope.rs:
  - `fn f alpha beta:\n\tal|` -> `alpha` is `Local`, ranked first.
  - The header `inst i cutoff: float = 1200 res: float = 0.3:` binds
    exactly `cutoff` and `res`, not `float`.
  - `map xs {x -> * x |}` -> `x` is a local.
  - `x ->:\n\t+ x |` -> `x`.
  - `for [k v] d:\n\t|` -> `k` and `v`.
  - `match v:\n\tvoice [note: p] ->:\n\t\t|` -> `p` (the key `note` is
    not bound).
  - A block `let y` defined after the cursor -> not visible.
  - `fn f a:\n\tlet y 1\n\t|` -> `y` is a `Local` (a block local defined
    before the cursor's statement is visible).
  - A block `let y` in a closed sibling block -> not visible.
  - At top level after a block -> the block's locals are not visible.
  - A tab-only cursor line `fn f a:\n\t|` -> `a`.
  - Shadowing: `fn f a:\n\tfn g a:\n\t\t|` -> exactly one `a`, with the
    detail of the inner fn.
- rank.rs:
  - With the prefix `de`, a prefix match ranks before a subword match
    (`lpg-decay`) and before a fuzzy match.
  - Within class 0, local before document before prelude.
  - Two runs give an identical `Vec<Candidate>`.
  - The empty prefix gives `items.len() == 100` and `incomplete == true`.
  - `limit = 3` gives 3 items.
  - `s :|` -> sounds and synths before controls, and `:analog` and
    template names are present.
  - `s :analog > cu|` -> the first item is `cutoff` (kind `Control`).
  - The header keys of a document `fn f a gain: 0.5:` are offered as
    `gain:` in `f 1 g|` once the arity is reached.
  - The header keys of a document `inst v cutoff: 1.0 color: 0.5:` are
    offered in tier 0 as `color:` for the cursor text `v cutoff: 1 co|`
    (a document `inst` has no positional parameters here, so its key set
    is reached via the earlier-pair rule). For `v c|` (no earlier pair,
    unknown arity) the context is `Argument`, and no `cutoff:` key is
    offered.
  - A `Snapshot` with `packages = [drums -> [kick, snare]]`: `drums.s|`
    -> `drums.snare`.
- robust.rs:
  - Every char-boundary prefix of every committed `.vact` file and every
    `src/fmt/tests/fixtures/*.in` completes without panicking.
  - 2000 xorshift mutants, with the mutation tokens of
    `fmt/tests/mutation.rs`, never panic.
  - The cursor at `len + 5` is clamped.
  - A cursor inside a multi-byte char (a Japanese comment line) steps
    back.
  - `complete_bytes`:
    - `[0xff]` -> 2;
    - a cursor past the end -> 3;
    - a valid input -> 0, and the JSON parses with `serde_json` and has
      the keys `v context from to incomplete items`.
  - `budget`: `#[ignore]`; a 2000-line document; the median of 20 calls
    is under 5 ms, and it prints the median.
  - `budget_tripwire`: not ignored; the same document; one call takes
    under 250 ms.

## Execution Protocol

- Follow `cmp-dispatch.json` `executionModel.editProtocol`.
  - Before each edit, re-read the file and record its sha256 and a
    one-line intent snapshot in the Progress Log. Record the sha256 after
    the edit.
  - On drift, stop and report.
  - Never edit outside writePaths. Make no git state changes.
- Write logs to `tmp/cmp/CMP-10/attempt-<n>/<k>-<name>.log`, each ending
  with `exit=<status>`.

## Verification (each must exit 0 with a complete log)

1. `CARGO_TERM_QUIET=true cargo build`
2. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
   (evidence: the core is wasm32-clean)
3. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run complete::`
   (a positive test count, all passing)
4. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --release --run-ignored only -E 'test(budget)'`
   Record the printed median. If it is 5 ms or more, record a finding.
   Do not loosen the bound.
5. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
6. `rustfmt --edition 2021 --check src/complete/*.rs src/complete/tests/*.rs src/lib.rs`
7. `wc -l src/complete/*.rs src/complete/tests/*.rs`: every file under
   1000 lines.

## Completion Criteria

- [ ] The pinned public contract exists exactly as written.
- [ ] All seven contexts, scope rules, sources and ranking are covered by
      the tests listed above, and they pass.
- [ ] No panic over the corpus prefixes and the mutants.
- [ ] The wasm32 build passes, and the release budget median is recorded.
- [ ] `src/lib.rs` has only the one-line module addition and its doc line.
- [ ] Verification steps 1-7 are logged with `exit=0`.
- [ ] The Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 226, step 4).
**Blockers**: None.

## Related Plans

- **Next**: CMP-20, CMP-21

### Session: 2026-09-30 (CMP-10 implementation)
**Tasks Completed**: Added the pinned public completion API and byte status seam; implemented cursor scanning, scope/source collection and ranking; added context, scope, rank and robustness tests; added the exact completion module doc bullet and `pub mod complete;` line in `src/lib.rs`.
**Edit evidence**: Before/after hashes and per-file intended behavior are recorded in `tmp/cmp/CMP-10/attempt-1/edit-intents.md`. The preserved fanout snapshot was read-only (sha256 `728efca8bac734a00b2204c046e997ce9b358b8dbb3e10e3ce09ae3174e044b6`).
**Verification**: An early `CARGO_TERM_QUIET=true cargo build` completed with exit=0 before implementation was complete; captured output is at `tmp/cmp/CMP-10/attempt-1/0-initial-build.log`. It is not final-source evidence. The plan gates 1-7 remain pending serial verification after this handoff.
**Completion Criteria**: API, source files, tests and `src/lib.rs` change are implemented; behavioral completeness and all gates await serial verification. No blocker identified.

### Session: 2026-09-30 (CMP-10 bounded repair)
**Tasks Completed**: Repaired the reported pair-colon, pipe seed-control, empty-limit, UTF-8 cursor test, for-pattern and function-parameter cases. Added document fn/inst key eligibility/source coverage, match pair-pattern binder coverage, formatter fixture prefix enumeration and the exact JSON wire-field/name assertion.
**Edit evidence**: Follow-up intents and literal pre-edit hashes are in `tmp/cmp/CMP-10/attempt-1/edit-intents.md`; final source hashes are recorded there after handoff. No shared paths were edited.
**Verification**: Per serial owner instruction, no verification commands were run during this follow-up. Fresh build, nextest and the CMP-10 plan gates remain pending.
**Completion Criteria**: All requested bounded repairs and added tests are implemented. Criteria requiring passing fresh behavioral and wasm/performance gates remain unchecked pending verification.

### Session: 2026-09-30 (CMP-10 criteria audit)
**Tasks Completed**: Added dynamic metadata and native PairKey assertions, including no-prior-pair arity fallback; added both for-pattern binders, sibling/outside block visibility, tab-only parameters and inner shadow detail assertions; made sound/synth/template keyword suborder precede controls; added explicit match-class, tier and label tie-break ordering assertions.
**Behavioral changes**: Scope descent now follows cursor indentation and function/instance details identify the owning declaration. Sound keyword controls remain available after sound/synth/template candidates.
**Edit evidence**: Exact audit intents and literal pre/post hashes are recorded in `tmp/cmp/CMP-10/attempt-1/edit-intents.md`.
**Verification**: No checks were run in this bounded audit; fresh checker and plan gates remain pending.

### Session: 2026-09-30 (CMP-10 explicit criteria assertions)
**Tasks Completed**: Added dynamic metadata and native PairKey tests that assert the selected key, tier precedence and pre-arity fallback; added explicit both-k/v, sibling/outside block, tab-only parameter and inner shadow-detail checks; added sound/synth/template-before-control ordering and rank class/tier/label tie-break assertions. Block traversal now follows the last preceding statement and cursor indentation so closed blocks do not leak.
**Edit evidence**: Per-edit intents and hashes are in `tmp/cmp/CMP-10/attempt-1/edit-intents.md`.
**Verification**: No checks run in this turn; fresh post-modification checker and seven gates are pending.

### Session: 2026-09-30 (CMP-10 final verification)
**Tasks Completed**: Final core and test implementation passed all seven assigned verification gates. The tests explicitly cover seven cursor contexts, native and metadata PairKey paths, scope binders and visibility, candidate sources and order, ranking classes/tiers/ties/cap, JSON/status wire contract, corpus/fixture prefixes and 2,000 mutants.
**Verification (final source)**:
1. `CARGO_TERM_QUIET=true cargo build` — exit=0; `tmp/cmp/CMP-10/attempt-1/1-cargo-build-final4.log`.
2. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` — exit=0; `tmp/cmp/CMP-10/attempt-1/2-wasm-build-final4.log`.
3. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run complete::` — exit=0, 16 passed, 1657 skipped; `tmp/cmp/CMP-10/attempt-1/3-nextest-final4.log`.
4. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --release --run-ignored only -E 'test(budget)'` — exit=0; median 1.071 ms from the quiet-cargo no-capture run; `tmp/cmp/CMP-10/attempt-1/4-release-budget-final4.log`, latest median evidence `tmp/cmp/CMP-10/attempt-1/4-release-budget-median-final5.log`.
5. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` — exit=0; `tmp/cmp/CMP-10/attempt-1/5-clippy-final4.log`.
6. `rustfmt --edition 2021 --check src/complete/*.rs src/complete/tests/*.rs src/lib.rs` — exit=0; `tmp/cmp/CMP-10/attempt-1/6-rustfmt-final4.log`.
7. `wc -l src/complete/*.rs src/complete/tests/*.rs` — exit=0; every Rust file is below 1000 lines; `tmp/cmp/CMP-10/attempt-1/7-line-count-final4.log`.
**Additional check**: `CARGO_TERM_QUIET=true cargo check` — exit=0; final-source log `tmp/cmp/CMP-10/attempt-1/1-cargo-check-final4.log`.
**Evidence**: Edit intents and pre/post hashes are in `tmp/cmp/CMP-10/attempt-1/edit-intents.md`. Persisted clippy, rustfmt and line-count wrapper failures are retained with final successful reruns. Two early agent-mediated nextest failures were reported before a complete plan-local log was requested; their reported compile/test failures were repaired, and final passing nextest runs have complete logs.
**Completion Criteria**: Implementation and all plan-owned verification are complete. Formal review and downstream plans CMP-20/CMP-21 remain separate workflow steps.
