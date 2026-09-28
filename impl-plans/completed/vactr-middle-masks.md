# Vactr Middle End: Types, Masks, Native Table and Codes (ME-MASKS) Implementation Plan

**planId**: ME-MASKS (first deliverable of vactr-core.md TASK-004; TASK-005's compiler consumes it)
**Status**: Completed (implemented, gate-verified, adversarial review 0 blocking, integration review accepted in sessions 175/176; removed from the dispatch manifest by the session-177 amendment; source rides in the single workflow commit; archiving to impl-plans/completed/ after the workflow commit, on user confirmation)
**Design Reference**: design-docs/specs/design-implementation.md sections 5.5, 7, 7.1.2, 7.1.3 (Masks, Native signature table, Codes), 7.1.4, 7.1.6
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactr/issues/2
**dependsOn**: none (wave 1)
**Dispatch manifest**: impl-plans/active/me-middle-20260925-s175-dispatch.json

---

## Intent and Context

Issue #2 implements the Vactr middle end: the checker (TASK-004), namespace, compiler and VM (TASK-005), and the pattern
engine (TASK-006). This plan lands the contracts that every later wave imports, so the parallel waves never redefine them:
- the checker type representation (`Ty`, `Scheme`, `CheckEnv`);
- forcing masks and their inference (`MaskEntry`, `ForcingMask`, `CalleeRef`, `infer_masks`, the `Forward` chase and
  the `mixed-forcing` set check). This is design 5.5: mask derivation is a MANDATORY compile dependency;
- the native signature table (one entry per prelude name in scope, 7.1.3);
- every `DiagCode` and `FailCode` in design 7.1.6, added in ONE edit. After this wave no other wave edits
  `src/types/diag.rs` or `src/vm/fail.rs` (7.1.3 Codes).

The input is the TASK-001..003 tree (commit 0bee1fb plus the checked-in design). Design 7.1 is normative. Where this plan
and the design differ, the design wins and the difference goes in the progress log.

## Non-Goals

- No unification, inference of whole programs, or checker diagnostics (ME-CHECK).
- No compiler, VM, namespace or native implementations (ME-VM, ME-REACTIVE, ME-INTEGRATE).
- No reader or value changes (ME-FRONTEND). `Ty::Path`, `Ty::Url`, `Ty::Sound` are type-level only here.
- No synthesis/effect/bus/granular vocabulary and no package loading in the native table (7.1.3: out of scope, they stay
  `undefined-name`).

## writePaths (exclusive)

- `src/types/ty.rs`, `src/types/masks.rs`, `src/types/natives.rs`, `src/types/natives_domain.rs`
- `src/types/diag.rs` (codes and the inline code-count test only)
- `src/vm/fail.rs` (codes and a code-count test only)
- `src/types/tests/mod.rs`, `src/types/tests/ty.rs`, `src/types/tests/masks.rs`, `src/types/tests/natives.rs`
- `impl-plans/active/vactr-middle-masks.md` (this plan; progress log)

## sharedPaths

- `src/types/mod.rs`: add `pub mod ty; pub mod masks; pub mod natives; mod natives_domain;` and `#[cfg(test)] mod tests;`.
  Keep `pub mod diag;`.

## File-Level Changes (signatures only; no code in this plan)

- `ty.rs`:
  - `Ty { Int, Int64, Float, Float64, Ratio, Bool, Str, KeywordOf(KeySet), Nil, Opt(Box<Ty>), List(Box<Ty>),
    Dict(Box<Ty>, Box<Ty>), Fn(Box<[Ty]>, Box<Ty>), Pattern(Box<Ty>), Signal, Path, Url, Sound, Named(TypeId), Any,
    Var(TyVar), NumLit(NumKindVar) }` (the last is the numeric-literal kind variable of design 7).
  - `KeySet` (a sorted set of keyword names, `Open` for "any keyword"), `TyVar(u32)`, `TypeId(u32)`,
    `Scheme { vars: Box<[TyVar]>, ty: Ty }`.
  - `BindKind { Let, Var, Fn, Inst, Struct, Enum, Param, PatternBinding }`.
  - `GlobalInfo { kind: BindKind, scheme: Option<Scheme>, mask: Option<ForcingMask>, span: Option<Span> }`.
  - `CheckEnv { globals: BTreeMap<Rc<str>, GlobalInfo>, qualified: BTreeMap<Rc<str>, BTreeMap<Rc<str>, GlobalInfo>>,
    opens: Vec<(Rc<str>, BTreeSet<Rc<str>>)> }` with `CheckEnv::empty()`. `globals` is the session state before the
    checked program (7.1.4: a top-level binding of a name already in `globals` is redefinition, not `rebinding`).
- `masks.rs` (design 5.5):
  - `CalleeRef { Native(NativeId), Global(Rc<str>), Qualified { prefix: Rc<str>, name: Rc<str> } }`.
  - `MaskEntry { Value, Fn, Late, Forward { links: Box<[(CalleeRef, u16)]> }, Undetermined }`.
  - `ForcingMask(pub Box<[MaskEntry]>)`.
  - `EffectiveEntry { Value, Fn, Late, Undetermined }` and
    `chase(links: &[(CalleeRef, u16)], lookup: &dyn Fn(&CalleeRef) -> Option<ForcingMask>) -> (EffectiveEntry, bool /* mixed */)`:
    the 5.5 combination rule (any `Value` -> `Value`; all `Fn`/`Late` -> `Fn` if any `Fn` else `Late`; any
    `Undetermined`, unresolvable link, or chase cycle (visited set) -> `Undetermined`). `mixed` is true when one link
    resolves to `Value` and another to `Fn`/`Late`. The VM calls this at the call boundary; nothing else re-implements it.
  - `infer_masks(f: &Node, env: &CheckEnv) -> ForcingMask` for an expanded `fn` or lambda node: a parameter in call
    position or with a function-type annotation is `Fn`; a parameter consumed by a `Value`-masked native is `Value`; a
    parameter passed on to another callee's parameter is `Forward` with one link per destination; a parameter reaching
    only `any` or an unknown callee is `Undetermined`; an unused parameter is `Value` (the decided "ignored value
    argument is forced once").
  - `mixed_forcing(mask: &ForcingMask, lookup: ...) -> Vec<u16>`: parameter positions whose `Forward` links currently
    disagree (used by ME-CHECK for the `mixed-forcing` warning).
  - Depth guard: past 1024 nested nodes, stop descending and mark the remaining parameters `Undetermined` (no panic).
- `natives.rs` and `natives_domain.rs` (7.1.3):
  - `NativeSig { name: &'static str, kind: NativeKind /* Function | Value */, min_args: u8, max_args: Option<u8>, keywords: &'static [&'static str],
    schemes: fn() -> Vec<Scheme>, mask: &'static [NativeMask], effectful: bool, needs: &'static [HostCap] }`, with
    `NativeMask { Value, Fn, Late }` and `NativeMask::entry() -> MaskEntry` (a `const`-friendly static mask, because
    `MaskEntry::Forward` holds a `Box`). Native masks are only `Value`, `Fn` or `Late` (natives never `Forward`).
    `NativeSig::forcing_mask() -> ForcingMask` converts it. `schemes` has more than one entry only for
    the fixed overload group `scale` and `shape` (M1, 7.1.4), in subject-type order.
  - `HostCap { MidiIn, MidiOut, Analysis, Render }` (for `beyond-capability`, ME-CHECK).
  - `NativeTable` with `get(name) -> Option<(NativeId, &NativeSig)>`, `sig(NativeId)`, `iter()`, and `NativeTable::global()`.
    `NativeId` is the existing `crate::value::value::NativeId`; ids are dense in table order.
  - Entries: the lang-reference section 5 core prelude; the design-music pattern, control and signal vocabulary (table at
    design-music.md lines ~409-410 and the control list at ~145-147); `d1`..`d9`, `slot`, `once`, `at`, `hush`, `stop`,
    `use-bpm`, `use-cycle`, `use-clock`, `midi-notes`, `cc`; `s`/`sound` (keyword parameter `kit`, type
    `[keyword: sound]`, 7.1.4), `sample : fn path -> sound`, `midi : fn int -> sound`, `load : fn path -> 'a`;
    `default-sound-kit` and `sound-kit` as prelude VALUE entries of type `[keyword: sound]`; the design-visual vocabulary.
    `range` is subject first (M2). `grid` exists; `struct` is not a pattern function (Q3). `gain` is a control (Q4).
    Core entries go in `natives.rs`; domain entries (pattern, control, signal, sound, visual) in `natives_domain.rs`,
    so neither file reaches 800 lines.
- `diag.rs`: add these `DiagCode`s (27 new; `ALL.len()` becomes 65), grouped with comments as in 7.1.6:
  reader `bad-path`, `bad-url`; `type-mismatch`, `annotation-mismatch`, `optional-as-value`, `any-not-narrowed`,
  `undefined-name`, `rebinding`, `shadowing`, `shadows-prelude`, `upd-immutable`, `literal-division-by-zero`,
  `unknown-keyword`, `missing-variant`, `bare-variant-binding`, `sound-not-first`; `duplicate-key`, `import-collision`,
  `beyond-capability`, `effect-in-pattern`, `unbounded-source`; `mixed-forcing`, `latent-forcing`; `bad-slice-points`,
  `input-lane-operator`, `clock-source-unavailable`; `dependency-cycle`. Add `DiagCode::default_severity()` returning
  `Warning` for `shadowing`, `duplicate-key`, `import-collision`, `effect-in-pattern`, `unbounded-source`,
  `mixed-forcing`, `latent-forcing`, `Hint` for `shadows-prelude`, and `Error` otherwise. Update the count test to 65.
- `fail.rs`: add `FailCode`s `no-match`, `fuel-exhausted`, `depth-exceeded`, `effect-in-query`, `effect-in-rebuild`,
  `undefined-name`, `not-callable`, `arity`, `blocked`, `slice-index`, `bad-slice-points`, `no-whole`, `upd-immutable`,
  `unknown-sound`, `host-unavailable`, `load-failed` (16 new, 20 total) with kebab-case `as_str`, plus a `FailCode::ALL`
  list and a test asserting 20 unique kebab-case names.

## Required Tests (src/types/tests/)

- `masks.rs`: `fn f x: + x 1` gives `[Value]`; `fn f g: g 1` gives `[Fn]`; `fn later body: at 4 body` gives
  `[Forward{(at,1)}]` (the link position `at` expects); a parameter forwarded to two callees gives two links; a parameter
  used only through an `any`-typed call gives `Undetermined`; an unused parameter gives `Value`; `chase` on
  {Value, Fn} returns `(Value, true)`, on {Fn, Late} returns `(Fn, false)`, on {Late} returns `(Late, false)`, on a
  cycle or an unknown link returns `(Undetermined, _)`; a 1100-deep nested body does not panic.
- `natives.rs`: names are unique; every `NativeSig` has a mask length within `max_args`; `scale` and `shape` have exactly
  two schemes; `s` has keyword `kit`; `load`, `sample`, `midi`, `default-sound-kit`, `sound-kit`, `grid`, `range`, `gain`
  are present and `struct` is absent; the domain constructors that design 5.5 (line ~380) says capture thunks and
  `VarRef`s deferred carry `NativeMask::Late` on those parameters: at least every design-visual source/transform
  numeric parameter (`osc` all three, `rotate`), and the pattern/signal constructors' numeric
  `PParam` positions (`fast`, `maybe`, `degrade-by`, `range`), with a test asserting `osc`'s mask is all `Late`; a
  listed-vocabulary test copies the in-scope names from the spec tables (cite the
  document line numbers in the test comment) and asserts each is present.
- `ty.rs`: `Ty` equality and `KeySet` membership basics.
- Inline code tests: `DiagCode::ALL.len() == 65`, `FailCode::ALL.len() == 20`, names unique and kebab-case, and
  `default_severity` for one code of each severity.

## Invariants

- The mask and native types are defined ONLY here. Later waves import them; a later wave that finds something missing
  records a finding and the INTEGRATE wave adds it (7.1.3). `diag.rs` and `fail.rs` are not edited again in this issue
  except by ME-INTEGRATE for a missing code (recorded).
- No `.rs` file reaches 800 lines (7.1.2). No panic on any input. No `std::{thread,fs,time,net,process}`. No dependency.
- Existing tests keep passing. The two existing code-count assertions change only in the number.

## Edit Protocol (common to every ME plan)

1. Before each edit, read the file fresh and record `shasum -a 256 <file>` (pre-edit) in
   `tmp/me-middle-20260925-s175/ME-MASKS/attempt-<n>/pre-edit-hashes.txt`.
2. Before editing a sharedPath, write a one-line intent snapshot (file, hunk purpose) to `.../intent.md`. After the edit,
   record the post-edit hash in `.../post-edit-hashes.txt`.
3. If a pre-edit hash differs from the last recorded post-edit hash for that file, stop, re-read, and reapply the intent
   instead of overwriting (drift detection). Never revert another plan's hunk.
4. Keep the tree compiling between edits: declare a new module in `mod.rs` only once it compiles. If the build breaks in a
   file outside this plan's writePaths/sharedPaths, do not edit that file; wait, re-run, and record the event. Serial
   repair of cross-plan breakage happens after the join (reconcile step), not here.
5. Run `rustfmt --edition 2021` on owned files only; never `cargo fmt` crate-wide (ME-FINAL does that).
6. Never run `git reset`, `git clean`, `git stash`, `git checkout -- <path>`, never create branches or worktrees, and
   never commit. The single implementation commit is made by the workflow commit step (ME-FINAL documents the staging list).
7. At the end, write `.../final-hashes.txt` with the sha256 of every file this plan wrote.

## Verification (foreground; design 6.5.7 evidence rule)

`mkdir -p target/fe-logs` first. `LOG` is a new file `target/fe-logs/masks-<check>-s<S>-<n>.log` (`<S>` = the Riela
session number, `<n>` counts from 1 per check within the session; never overwrite). Run each cargo row as
`(set -o pipefail; CMD 2>&1 | tee LOG); echo "exit=$?" >> LOG`. The progress log cites each counting log path (last run
after the final code change) and its `exit=` value; V3 and V3t also cite run/passed counts (non-zero). A missing log, a
log without `exit=`, or a truncated log fails the row. `\|` is a literal `|`.

| # | Command (`CMD`) | `<check>` | Evidence |
|---|---------|-----------|----------|
| V1 | `CARGO_TERM_QUIET=true cargo build` | `build` | `exit=0`, no warnings |
| V2 | `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` | `clippy` | `exit=0` |
| V3 | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run` | `nextest` | `exit=0`, run > 0, 0 failed |
| V3t | `CARGO_TERM_QUIET=true cargo test` | `cargotest` | `exit=0`, non-zero test count (the workflow gate recognizes `cargo test`) |
| V3f | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'binary(spec_fixtures)'` | `fixtures` | `exit=0`, run > 0, 0 failed |
| V6a | `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown` | `wasm32` | `exit=0` |
| V6b | `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm` | `wasm32-hostwasm` | `exit=0` |
| V4 | `find src tests -name '*.rs' -exec wc -l {} + \| sort -n \| tail -5` | - | every file under 800 lines (inline) |
| V5 | `grep -rnE 'std::(thread\|fs\|time\|net\|process)' src/ \|\| echo none` | - | prints `none` |
| V7 | `git diff --stat -- Cargo.toml Cargo.lock` | - | empty |
| V8 | `rustfmt --edition 2021 --check <owned .rs files>` | - | exit 0 (inline) |

## Completion Criteria

- [x] `Ty`, `Scheme`, `KeySet`, `BindKind`, `GlobalInfo`, `CheckEnv` exist in `src/types/ty.rs` as listed
- [x] `MaskEntry`, `ForcingMask`, `CalleeRef`, `EffectiveEntry`, `chase`, `infer_masks`, `mixed_forcing` exist in `src/types/masks.rs` with the listed tests passing
- [x] The native table has every in-scope entry (7.1.3) including `s` with `kit`, `load`, `sample`, `midi`, `default-sound-kit`, `sound-kit`, and the `scale`/`shape` overload group
- [x] Domain constructors that capture deferred arguments (design 5.5, line ~380) carry `Late` entries; `osc`'s mask is asserted all `Late`
- [x] `DiagCode::ALL.len() == 65` and `FailCode::ALL.len() == 20`, with `default_severity`
- [x] V1-V8 pass with logs cited in the progress log; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ME-MASKS implementer)` entry with Tasks Completed, design
differences, hashes/intent paths, verification evidence per row, and blockers. Edit only this plan's log.)

### Session: 2026-09-25 (session 175, ME-MASKS implementer)

**Tasks Completed**: every ME-MASKS deliverable. New: `src/types/ty.rs` (420 lines), `src/types/masks.rs` (698),
`src/types/natives.rs` (375), `src/types/natives_domain.rs` (277) and `src/types/tests/{mod,ty,masks,natives}.rs`.
Edited: `src/types/diag.rs` (27 codes, now 65, plus `default_severity`), `src/vm/fail.rs` (16 codes, now 20, plus
`FailCode::ALL` through a `fail_codes!` macro like `diag_codes!`, and a count test) and the sharedPath
`src/types/mod.rs` (`pub mod ty; pub mod masks; pub mod natives; mod natives_domain; #[cfg(test)] mod tests;`).
The native table has 200 entries.

**Design and plan differences** (design 7.1 wins; ME-CHECK, ME-VM and ME-INTEGRATE should read these):
- `NativeSig` stores its schemes as notation strings, `ty: &'static [&'static str]`, parsed by `Scheme::parse`. The
  accessor is the method `schemes()` instead of the planned `fn() -> Vec<Scheme>` field. A test parses every entry.
- `Ty::Tex` (visual chain) was added. The M1 `scale` overload needs a texture subject type, and design 7 lists none.
- Rest convention: for a variadic native, the last parameter type and the last mask entry apply to every further
  argument. `ForcingMask::entry(pos)` applies the same rule. A parameter past `min_args` is optional. An overloaded
  scheme may have fewer parameters than the mask (`shape`: 3 or 2).
- `CalleeRef`: a `Sym` head that resolves to the prelude links as `Global(name)`, so a session redefinition of `at` is
  chased (5.5). Operator and expander `Builtin` heads link as `Native(id)`. A `Value`-masked native position decides
  `Value` directly, as design 5.5 says.
- `chase` precedence: any `Value` -> `Value`, then any `Undetermined`, then `Fn`, then `Late`. Design 5.5 does not
  order `Value` against `Undetermined`. Forcing once at the boundary keeps exactly-once semantics. A path-local visited
  set, depth 64 and a 4096-link budget bound the chase. `mixed` covers the top-level link set only.
- `infer_masks`: a bare parameter on a statement line of its own is `Fn`. This follows lang-reference section 1
  `maybe-do`: "`body` on a line of its own" is called. `Fn` > `Value` > `Undetermined` > `Forward`. A named argument
  or a splatted value is `Value`. A positional argument after a splat is `Undetermined`. A list-pattern parameter is
  `Value`.
- Extra public API: `header_params` (the one header parser, `HeaderParam`; mask order is header order, keyword
  parameters included), `static_mask` (a checker-side lookup for `mixed_forcing`), `CheckEnv::{global, qualified,
  open_prefix}`, `NativeTable::{len, is_empty, name_count}`, `NativeSig::{entry_at, is_overloaded}`.
- Arithmetic and math schemes are `'a`-generic (`+ : fn 'a 'a -> 'a`, `sin : fn 'a -> 'a`), so `{sin time}` over a
  signal checks. The numeric lattice is ME-CHECK's unifier. Domain parameters are `any`: number | signal | pattern |
  fn of time (design-visual section 1).
- `shape` masks position 0 as `Value`, because the VM dispatches the overload on the runtime tag, which a thunk would
  hide. `add`/`sub` take a generic subject because they are also pattern arithmetic (`add p 7`, design-music section 3).
- Absent on purpose. `osc` as OSC output (design-music line 417): the entry is the visual source. `bus`, `master`,
  synthesis, effects and granular are TASK-008. Also absent: `parse`, the word aliases (`gt`, `lt`, ...), and the
  inst-parameter controls `cutoff`/`position`/`bank` (M3, TASK-008). The checker reports these as `undefined-name`
  until a later wave records a finding. `o0`..`o3` are prelude values of type `keyword`.

**Environment event**: the machine volume was full (about 120 MiB free), and a rustc ICE followed (os error 28). Only
`target/debug/incremental` (a regenerable cache) was removed. Every cargo row below ran with `CARGO_INCREMENTAL=0` added.

**Evidence** (`tmp/me-middle-20260925-s175/ME-MASKS/attempt-1/`): `pre-edit-hashes.txt`, `intent.md`,
`post-edit-hashes.txt`, `final-hashes.txt`. No drift was detected on `src/types/mod.rs`, `diag.rs`, `fail.rs` or this plan.

**Verification** (logs are under `target/fe-logs/`, each run after the final code change, `CARGO_INCREMENTAL=0` added):
- V1 `masks-build-s175-1.log`: `exit=0`, no warnings.
- V2 `masks-clippy-s175-1.log`: `exit=0`.
- V3 `masks-nextest-s175-1.log`: 189 tests run, 189 passed, 0 failed, `exit=0`.
- V3t `masks-cargotest-s175-1.log`: lib 181 passed, spec_fixtures 8 passed, 0 failed, `exit=0`.
- V3f `masks-fixtures-s175-1.log`: 8 run, 8 passed, `exit=0`.
- V6a `masks-wasm32-s175-1.log`: `exit=0`. V6b `masks-wasm32-hostwasm-s175-1.log`: `exit=0`.
- V4: largest files `src/reader/line.rs` 741, `src/types/masks.rs` 698, `src/reader/lexer.rs` 690 (all under 800).
- V5: `none`. V7: empty (no dependency change). V8: `rustfmt --edition 2021 --check` on the 11 owned files, exit 0.

**Blockers**: none. Formal test-integrity/adversarial review, crate-wide fmt (ME-FINAL) and the single workflow
commit are downstream.

## Related Plans

- **Parent**: impl-plans/active/vactr-core.md (TASK-004 first deliverable)
- **Next**: vactr-middle-frontend.md (ME-FRONTEND), then ME-CHECK, ME-VM, ME-PATTERN in parallel
