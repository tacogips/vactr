# Vactrol Front End: Value Model and Foundation (FE-VALUE) Implementation Plan

**planId**: FE-VALUE (implements vactrol-core.md TASK-001)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md sections 4, 5.1-5.4, 6.5.1-6.5.3, 6.5.7
**Created**: 2026-09-25
**Last Updated**: 2026-09-25
**dependsOn**: none (wave 1). Precondition: design section 6.5 and all FE-* plans are committed on `main`.

---

## Intent and Context

The user asked (in Japanese) for the Vactrol language front end to be implemented through the
design-and-implementation workflow. This plan is the first wave. It
replaces the scaffold (`src/lib.rs` holds a placeholder `hello()`, and
`src/main.rs` calls it) with the crate skeleton, the foundation id types,
placeholder shells, the diagnostic and failure types, and the complete
core value model. FE-READER (wave 2) and FE-EXPAND (wave 3) build on these
types. Toolchain: Rust 1.83 via mise, edition 2021, so let-chains are not
available. The crate has no dependencies and must keep none.

## Non-Goals

- No checker, VM, compiler, pattern engine, scheduler, DSP, session, LSP or editor behavior.
- No arithmetic natives (`+`, `/`, and so on). They belong to TASK-005. Only the widening join and `widen` are in scope.
- No methods, constructors or trait impls on shells beyond `Debug`.
- No edits to `impl-plans/active/vactrol-core.md` or `impl-plans/README.md`. FE-FINAL owns those files.
- No toolchain change. Do not run `rustup target add` (design-docs/user-qa/pending-frontend-questions.md U1).

## writePaths (exclusive to this plan)

- `Cargo.toml`: add `[features] default = ["host-native"]`, and `host-native = []`, `host-wasm = []`, `lsp = []`. Add no dependencies.
- `src/main.rs`: print `vactrol <CARGO_PKG_VERSION>` only.
- `src/lib.rs`: crate doc, and `pub mod value; pub mod reader; pub mod types; pub mod ns; pub mod compile; pub mod vm; pub mod pattern; pub mod tex; pub mod sched; pub mod dsp;` (no `expand` yet; FE-EXPAND adds it). Remove `hello()` and its test.
- `src/value/mod.rs`, `value.rs`, `ratio.rs`, `key.rs`, `dict.rs`, `intern.rs`, `num.rs`, `access.rs`, `eq.rs`, `print.rs`.
- `src/value/tests/mod.rs`, plus one test file per topic: `ratio.rs`, `key.rs`, `dict.rs`, `num.rs`, `access.rs`, `eq.rs`, `print.rs`, `intern.rs`.
- `src/reader/span.rs`.
- `src/types/mod.rs` and `diag.rs`; `src/vm/mod.rs` and `fail.rs`.
- `src/ns/mod.rs`, `namespace.rs` and `tweak.rs`; `src/compile/mod.rs` and `proto.rs`.
- `src/pattern/mod.rs`, `pat.rs` and `signal.rs`; `src/tex/mod.rs` and `texnode.rs`.
- `src/sched/mod.rs` and `slots.rs`; `src/dsp/mod.rs` and `graph.rs`.

## sharedPaths

- `src/reader/mod.rs`: created here containing only `pub mod span;`. FE-READER extends it later.
- `src/lib.rs`: FE-EXPAND later adds one line, `pub mod expand;`.

## File-Level Changes (signatures only, no code)

### Foundation ids (design 6.5.1)
Each id is a newtype deriving `Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug`, with
`pub const fn new(raw) -> Self` and `pub const fn get(self) -> raw`.
- `src/reader/span.rs`: `FileId(u32)` with `pub const CONSOLE: FileId` (id 0); `NodeId(u32)`;
  `Span { pub file: FileId, pub start: u32, pub end: u32 }` with `new`, `len`, `join(self, other) -> Span`
  (the covering span; both spans are in the same file); `SrcRef { pub span: Span, pub doc_revision: u64, pub form_gen: FormGen }`.
- `src/value/intern.rs`: `KwId(u32)`, `SymId(u32)`, `Interner` (below).
- `src/ns/namespace.rs`: `FormGen(u64)`, and the shell `pub struct VarSlotRef { _private: () }`.
- `src/ns/tweak.rs`: `TweakId(u32)`.
- `src/compile/proto.rs`: the shell `pub struct Closure { _private: () }`.
- `src/pattern/pat.rs`: shell `Pat`. `src/pattern/signal.rs`: shell `Sig`.
- `src/tex/texnode.rs`: shell `TexNode`, and `OutId(u32)`.
- `src/sched/slots.rs`: `SlotId(u32)`, `CtlId(u16)`. `src/dsp/graph.rs`: `InstId(u32)`.
- Every shell derives only `Debug`, has a private unit field, and has no constructor.

### Failure (src/vm/fail.rs, design 8.4)
- `pub enum FailCode { DivisionByZero, Overflow, Type, UnknownField }`
- `pub struct Origin { pub span: Option<Span>, pub slot: Option<KwId>, pub beat: Option<Ratio64> }` with `Origin::none()`.
- `pub struct Failure { pub code: FailCode, pub message: String, pub origin: Origin }` with `Failure::new(code, message)`.
  Implement `Display` and `std::error::Error` by hand (no thiserror).

### Diagnostics (src/types/diag.rs, design section 7 and 6.5.4/6.5.5)
- `pub enum Severity { Error, Warning, Hint }`
- `pub struct RunOrigin { pub slot: Option<KwId>, pub beat: Option<Ratio64> }`
- `pub struct Diagnostic { pub span: Span, pub severity: Severity, pub code: DiagCode, pub message: String, pub origin: Option<RunOrigin> }`,
  with `Diagnostic::error(code, span, message)`, plus `Display` and `Error`.
- `pub enum DiagCode` with exactly these variants. `as_str()` returns the kebab-case name shown in parentheses.
  - Reader: SourceTooLarge(source-too-large), BadNumber(bad-number), ConsoleRegisterInFile(console-register-in-file),
    BadIdentifier(bad-identifier), ParenForm(paren-form), StrayChar(stray-char), UnterminatedString(unterminated-string),
    BadEscape(bad-escape), MisplacedColon(misplaced-colon), IndentSpace(indent-space), EmptyBlock(empty-block),
    ContinuationAfterBlock(continuation-after-block), UnexpectedIndent(unexpected-indent),
    ImportNotTopLevel(import-not-top-level), BadImport(bad-import), BindingWithoutValue(binding-without-value),
    ArrowWithoutBody(arrow-without-body), MisplacedArrow(misplaced-arrow; `->` inside `[..]`),
    MisplacedFallback(misplaced-fallback), BadPair(bad-pair), MisplacedSplat(misplaced-splat), EmptyGroup(empty-group),
    UnclosedGroup(unclosed-group), UnclosedBracket(unclosed-bracket), UnboundQualifier(unbound-qualifier),
    NestingTooDeep(nesting-too-deep).
  - Expander: ReadErrorPresent(read-error-present), SugarInPattern(sugar-in-pattern), ReservedWord(reserved-word),
    IfGuard(if-guard), MalformedIf(malformed-if), ElseWithoutIf(else-without-if), MalformedFor(malformed-for),
    MalformedMatch(malformed-match), MalformedFn(malformed-fn), MalformedBinding(malformed-binding), MalformedEnum(malformed-enum).
  `MisplacedArrow` is the code this plan chooses for the design 6.5.4 rule "`->` inside a list is a diagnostic".
  FE-FINAL records it in the design.

### Value model (design 5.1-5.4, 6.5.1-6.5.3)
- `value.rs`: `Value` with exactly the 5.1 variants. Also `ListVal { pub items: Box<[Value]>, pub prov: Option<Rc<ListProv>> }`,
  `ListProv { pub form_gen: FormGen, pub doc_revision: u64, pub elems: Box<[Span]> }`,
  `StructVal { pub ty: SymId, pub fields: Box<[(KwId, Value)]> }` and
  `VariantVal { pub enum_ty: SymId, pub tag: SymId, pub fields: Box<[(KwId, Value)]> }` (fields sorted by key name),
  `NativeId(u32)`, and `RangeVal { pub start: i64, pub end: Option<i64> }`.
  Constructors: `Value::list(Vec<Value>)` (prov `None`), `Value::str(&str)`, `Value::kw(&str)`.
  `Value` derives `Clone` and `Debug` and is `!Send` because it holds `Rc`.
- `ratio.rs`: `Ratio64 { num: i64, den: i64 }` (private fields, `num()` and `den()` getters), with
  `new(n, d) -> Result<Self, Failure>` and `checked_add`, `checked_sub`, `checked_mul`, `checked_div`, each `-> Result<Self, Failure>`.
  Also `floor() -> i64`, `frac() -> Ratio64`, `to_f64()`, `is_integral()`, `from_decimal(&str) -> Option<Ratio64>`,
  `from_f64_exact(f64) -> Option<Ratio64>` (a dyadic value in range), `Ord` by exact comparison through i128, and
  `Display` (an integral value prints as an int). Use i128 intermediates. A result that does not fit in i64 is
  `Overflow`; this includes negating `i64::MIN`. A zero denominator is `DivisionByZero`.
- `key.rs`: `NumKey { Exact(Ratio64), Float(f64) }` and `Key { Num(NumKey), Kw(KwId), Str(Rc<str>) }`, with a total
  `Ord` per 6.5.3 (Exact before Float when they are equal as f64; `-0.0` becomes 0; keywords compare by name
  through the thread-local interner). Also `Key::from_value(&Value) -> Result<Key, Failure>` (NaN, or a value that is not a key, is `Type`)
  and `Key::to_value(&self) -> Value`.
- `dict.rs`: `dict_from_pairs(items: &[Value]) -> Result<BTreeMap<Key, Value>, Failure>` (a repeated key keeps the last pair),
  `put(coll: &Value, elems: &[Value]) -> Result<Value, Failure>`, `join(colls: &[Value]) -> Result<Value, Failure>`,
  and `pairs(d: &BTreeMap<Key, Value>) -> impl Iterator<Item = Value>` (2-lists in key order). Semantics as in 6.5.3.
- `intern.rs`: `Interner { names: Vec<Rc<str>>, map: HashMap<Rc<str>, u32> }` with `intern(&str) -> u32` and
  `resolve(u32) -> Option<Rc<str>>`. Also a thread-local instance behind free functions `intern_kw(&str) -> KwId`,
  `intern_sym(&str) -> SymId`, `name_of_kw(KwId) -> Rc<str>` and `name_of_sym(SymId) -> Rc<str>`. An unknown id returns
  the empty string rather than panicking.
- `num.rs`: `NumKind { Int, Int64, Float, Float64, Ratio }`; `NumKind::of(&Value) -> Option<NumKind>`;
  `join(a, b) -> NumKind`, which matches the design 6.5.3 table exactly; and `widen(&Value, NumKind) -> Result<Value, Failure>`,
  which converts upward only. Narrowing, or a value that is not a number, is `Type`.
- `access.rs`: `truthy(&Value) -> bool`, `index(&Value, &Value) -> Result<Value, Failure>`,
  `get(&Value, &Value) -> Result<Value, Failure>`, `len(&Value) -> Result<usize, Failure>` and
  `first(&Value) -> Result<Value, Failure>`, with the nil-punning rules of 6.5.3.
- `eq.rs`: `deep_eq(&Value, &Value) -> Result<bool, Failure>`, following 6.5.3.
- `print.rs`: `impl Display for Value`, with the 6.5.3 print format and a private quoted form for nested strings.
- `mod.rs`: declares the submodules and re-exports the public items above. Tests: `#[cfg(test)] mod tests;`.

## Invariants

- No `.rs` file reaches 1000 lines. Test files stay under 1000 lines each.
- No `unwrap`, `expect`, indexing panic or `unreachable!` is reachable from a public function with user-derived input.
- Core modules use none of `std::thread`, `std::fs`, `std::time`, `std::net` or `std::process`.
- `Cargo.toml` `[dependencies]` and `[dev-dependencies]` stay empty.
- No behavior on shells. No `Value` holding a shell is built in code or tests.

## Required Tests (in src/value/tests/*.rs)

- ratio: normalization (`2/4` becomes `1/2`, `1/-2` becomes `-1/2`); exact `+ 1/4 1/8 = 3/8`; `* 1/4 4` is integral and prints `1`;
  `/ 6 4` prints `3/2`; overflow on add and mul near `i64::MAX` gives `Overflow`; `new(1, 0)` gives `DivisionByZero`;
  dividing by zero gives `DivisionByZero`; `i64::MIN` cases give `Overflow`; `floor` and `frac` of negative values;
  `from_decimal("1.5") == 3/2`.
- num: all 15 join pairs; each allowed widen, including int to float at 2^24+1 (lossy, accepted), int to ratio, and ratio to float;
  every narrowing request is `Type`.
- key: numbers < keywords < strings; `1`, `1.0` and `1/1` are one key; `-0.0` equals `0`; a NaN key is `Type`; keyword order
  is alphabetical even when the ids were interned in reverse order; a list is not a key.
- dict: iteration in key order for keys inserted out of order; a duplicate literal key keeps the last pair; `put` on a dict replaces
  a key; `put` on a dict with a non-pair element is `Type`; `put` on a list appends; `put` on nil is `Type`; `join` of lists
  concatenates; `join` of dicts merges with the last value winning; `first` of a dict is the smallest pair.
- access: `truthy` (nil and false are falsy; 0, "" and [] are truthy); `index nil 3`, `get nil :k`, `len nil = 0` and `first nil`;
  an index out of range, or negative, is nil; a float index is `Type`; an unknown struct field is `UnknownField`.
- eq: numbers equal across widths; NaN is not equal to NaN; nested lists; an empty list equals an empty dict; a pair-list equals a dict in key order;
  structs are nominal; variants compare tag and fields; an `Inst` compares by id; a `Native` gives `Type`.
- print: `90.0`, `0.16666667` (f32 of 1/6), `3/8`, `:kick`, a string raw at top level and quoted when nested, and `[amp: 0.5 pan: -1]`.
- intern: round-trip, and one id for the same text.
- Tagging: construct each non-shell variant and match on it.

## Edit Protocol (drift safety, same branch and working directory)

1. Before each edit, read the file fresh and record `shasum -a 256 <file>` in the progress log.
2. Before editing a shared path, write a one-line intent snapshot in the progress log. After the edit, record the new hash.
3. If a pre-edit hash differs from the last recorded post-edit hash, stop, re-read, and reapply the intent instead of overwriting.
4. Do not run `cargo fmt` over the whole crate if files owned by other plans exist. Run `rustfmt` on this plan's files only.

## Verification (run in the foreground; keep logs under target/fe-logs/)

Run `mkdir -p target/fe-logs` first. For each row, run the command and record the exit status and log path in the progress log.
In the tables, `\|` stands for a literal `|`. For piped commands, the exit status is `${PIPESTATUS[0]}`.

| # | Command | Evidence required |
|---|---------|-------------------|
| V1 | `CARGO_TERM_QUIET=true cargo build 2>&1 \| tee target/fe-logs/value-build.log` | exit 0, no warnings in the log |
| V2 | `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings 2>&1 \| tee target/fe-logs/value-clippy.log` | exit 0 |
| V3 | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run 2>&1 \| tee target/fe-logs/value-nextest.log` | exit 0; the summary line shows 0 failed |
| V4 | `find src -name '*.rs' -exec wc -l {} + \| sort -n \| tail -5` | the largest file is under 1000 lines |
| V5 | `grep -rnE 'std::(thread\|fs\|time\|net\|process)' src/ \|\| echo none` | prints `none` |
| V6 | `rustup target list --installed` | if `wasm32-unknown-unknown` is listed, run `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm`; otherwise record "blocked: U1" |
| V7 | `git diff --stat -- Cargo.toml` | the only change is the `[features]` table |

## Completion Criteria

- [ ] Foundation ids, shells, Failure, Diagnostic and DiagCode exist at the listed paths
- [ ] `Value`, `Ratio64`, `Key`/`NumKey`, dict ops, `Interner`, `num`, `access`, `eq` and `print` are implemented per design 6.5.3
- [ ] Ratio ops are exact and self-reducing; i64 overflow and a zero denominator are `Failure`
- [ ] Dict iteration is in key order; a duplicate literal key keeps the last pair
- [ ] Every required test above exists and passes (V3)
- [ ] V1, V2, V4, V5 and V7 pass; V6 is run or recorded as blocked on U1

## Progress Log

### Session: (implementer fills in)
**Tasks Completed**:
**Hashes / intent snapshots**:
**Verification evidence**: V1-V7 exit codes and log paths
**Blockers**:

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-001)
- **Next**: impl-plans/active/vactrol-frontend-reader.md (FE-READER)
