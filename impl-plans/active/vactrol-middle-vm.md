# Vactrol Middle End: Namespace, Compiler, Bytecode VM, Tweak Sites, Staging (ME-VM) Implementation Plan

**planId**: ME-VM (implements the non-reactive part of vactrol-core.md TASK-005)
**Status**: Completed (implemented, gate-verified, adversarial review 0 blocking, integration review accepted in session 177; removed from the dispatch manifest by the session-178 amendment; source rides in the single workflow commit)
**Design Reference**: design-docs/specs/design-implementation.md sections 5.5 (forcing contract), 5.6 (VarRef load rules, scope model, tweak slots, form generations), 5.7 (PkgNs, lookup order), 7.1.1-7.1.6, 8 (bytecode, frames, failure), 10.4 (Query effect mode), 13 (tweak site tiers)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/2
**dependsOn**: ME-MASKS (`infer_masks`, `chase`, native table, codes), ME-FRONTEND (`Value::Path`/`Url`/`Sound` constants)
**Dispatch manifest**: impl-plans/active/me-middle-20260925-s175-dispatch.json

---

## Intent and Context

Live mode runs on a dynamically checked bytecode VM. This plan builds the runtime core: the `Namespace` with the
Decided scope chain (read-only prelude -> session -> fn/block), late-bound `VarRef` load rules, the compiler from expanded
kernel forms to `FnProto` bytecode (match compiled to pattern ops, keyword args, splats, forcing masks from
`types/masks.rs`), the VM (frames, fuel, depth and re-entry limits, call-boundary forcing with the 5.5 `Forward` chase,
memoized `Undetermined` cells, `EffectMode::Query`, failure unwind with `Origin`), tweak sites and tiers, package
namespaces, staged effects (`StagedEffect`, `EffectSink`, `RecordingSink`), and the core prelude natives. The reactive
dependency graph, `Evaluator` and `load` are ME-REACTIVE; pattern/signal/visual/sound natives and `QueryVm` are
ME-INTEGRATE.

## Non-Goals

- No `DepGraph`, pass journal, `Evaluator`, reactive propagation, or `load` (ME-REACTIVE).
- No pattern, signal, visual, music or sound natives, no `QueryVm` impl (ME-INTEGRATE). No checker call in the pipeline
  (ME-INTEGRATE inserts it).
- No slot table, scheduler, hosts or DSP (TASK-007/008). The only `EffectSink` is `RecordingSink`.
- No Frozen mode, JIT or unboxed code.

## writePaths (exclusive)

- `src/compile/compiler.rs`, `src/compile/matchc.rs`, `src/compile/proto.rs` (replace the `Closure` shell), `src/compile/sites.rs`
- `src/compile/tests/mod.rs`, `src/compile/tests/compile.rs`, `src/compile/tests/sites.rs`
- `src/vm/ops.rs`, `src/vm/frame.rs`, `src/vm/vm.rs`, `src/vm/call.rs`
- `src/vm/natives/mod.rs`, `src/vm/natives/num.rs`, `src/vm/natives/list.rs`, `src/vm/natives/dict.rs`,
  `src/vm/natives/value.rs`, `src/vm/natives/console.rs`, `src/vm/natives/effects.rs`
- `src/vm/tests/mod.rs`, `src/vm/tests/forcing.rs`, `src/vm/tests/query_mode.rs`, `src/vm/tests/failures.rs`,
  `src/vm/tests/natives.rs`, `src/vm/tests/tweak.rs`
- `src/ns/namespace.rs` (replace the `VarSlotRef` shell; keep `FormGen`), `src/ns/tweak.rs`, `src/ns/pkg.rs`, `src/ns/stage.rs`
- `src/ns/tests/mod.rs`, `src/ns/tests/namespace.rs`, `src/ns/tests/pkg.rs`, `src/ns/tests/stage.rs`
- `impl-plans/active/vactrol-middle-vm.md`

## sharedPaths

- `src/compile/mod.rs`, `src/vm/mod.rs`, `src/ns/mod.rs`: declare the new modules and test modules; keep existing items
  (`vm::fail`, `ns::namespace::FormGen`, `ns::tweak::TweakId`, `compile::proto::Closure` paths stay stable because
  `src/value/value.rs` imports them).

## File-Level Changes (signatures only; vactrol-core.md Module 4 sketch is the shape)

- `ns/namespace.rs`: `VarSlot { name: SymId, kind: SlotKind, value: RefCell<Value>, version: Cell<u64> }`,
  `SlotKind { Let, Var, Fn, Tweak, Prelude }`, `VarSlotRef(Rc<VarSlot>)` (derives `Clone`, `Debug`), `Namespace` with:
  `Namespace::new(prelude: Prelude)`; `Prelude` = read-only table of natives (from `NativeTable`) and prelude values
  (`Prelude::register_value(name, Value)` used by ME-INTEGRATE for `default-sound-kit`/`sound-kit`);
  `define(name, kind, value, FormGen) -> VarSlotRef` in the SESSION scope (a session binding of a prelude name creates a
  new session slot; it never writes the prelude); `redefine` (Live redefinition replaces the session slot value and
  bumps `version`); `lookup(name) -> Option<Resolved>` walking session -> open imports (most recent first) -> prelude;
  `session_value(name)` (used later by `QueryVm::sound_kit`); `write_var(slot, Value) -> Result<(), Failure>` failing
  `upd-immutable` for any non-`Var` slot including prelude slots.
- `ns/pkg.rs`: `PkgNs { id: PackageId, ns: Namespace }`, `ImportBinding { prefix: SymId, pkg: PackageId, open: bool }`,
  re-import replaces the whole `PkgNs`; qualified lookup `pads.warm`.
- `ns/tweak.rs`: `TweakSite { id, span, slot, initial, ty: NumTy, tier: SiteTier, form_gen }`, `SiteTier { Direct, Reeval, Manual }`,
  per-`FormGen` `TweakId` allocation; `write_tweak(id, Value)` updates the cell (direct tier observable without re-eval).
- `ns/stage.rs`: `StagedEffect { SlotBind { slot: SlotKey, value: Value }, Revoke(SlotKey), CellUpdate, TweakRefresh,
  Bindings(..), Tempo(..), OneShot { at: Option<Ratio64>, value: Value }, Console(Rc<str>) }`, `EffectBuffer` (collect
  per form; `release(&mut dyn EffectSink)` only when the whole form succeeded; `drop_all` on failure), `trait EffectSink`,
  `RecordingSink` (test sink, records in order). `SlotKey` is `d1`..`d9` or a `slot :name` keyword.
- `compile/proto.rs`: `Closure { proto: Rc<FnProto>, captures: Box<[Value]>, mask: ForcingMask }`, `FnProto` per the
  Module 4 sketch (`arity`, `code`, `consts`, `locals`, `spans`, `name`).
- `vm/ops.rs`: the Module 4 `Op` set (`LoadConst`, `LoadLocal`, `StoreLocal`, `LoadGlobal`, `LoadGlobalRef`, `LoadTweak`,
  `MakeList`, `MakeDict`, `MakeClosure`, `MakeThunk`, `Call`, `CallKw`, `CallValue`, `Force`, `Deref`, `Jump`, `TestLit`,
  `TestVariant`, `TestLen`, `SplitRest`, `BindField`, `JumpIfNoMatch`, `Fail`, `Pop`, `Ret`), plus `UpdLocal`/`UpdGlobal`
  if needed (record).
- `compile/compiler.rs` + `matchc.rs`: `compile(n: &Node, cx: &mut CompileCx) -> Result<Rc<FnProto>, Diagnostic>`; name
  resolution at compile time per the 5.6 scope model (locals innermost, then session, opens, prelude); top-level `var`
  and `fn` load `VarRef` (late), top-level `let` loads a snapshot; path/url literal constants become `Value::Path`
  (with the literal span's `FileId` for `./`/`../`, `None` for `/`/`~/`) and `Value::Url`; list literals carry
  `ListProv`; every `fn`/lambda gets its mask from `types::masks::infer_masks` (always, independent of diagnostics);
  depth > 1024 -> `nesting-too-deep` and the form is not run (7.1.5).
- `compile/sites.rs`: tweak-site classification for the Decided site set (pattern/control literals, top-level bindings,
  `inst` header default literals as `Direct`), tier from the form's structure (design 13).
- `vm/vm.rs`, `frame.rs`, `call.rs`: `Vm { stack, frames, fuel, depth_limit: 512, reentry_limit: 64, effect_mode }`,
  `Vm::run(proto, ns) -> Result<Value, Failure>`; fuel 1,000,000 per top-level evaluation (`fuel-exhausted`); frames >
  512 or re-entries > 64 -> `depth-exceeded`; at `Call` apply the CURRENT callee mask left to right exactly once per
  argument: `Value` forces, `Fn`/`Late` pass, `Forward` -> `masks::chase` at the boundary, `Undetermined` -> memo cell
  (runs at most once); `EffectMode::Query` with a scope guard: global writes, scheduling and staged effects fail
  `effect-in-query` with origin, `print` is captured, frame-local state stays legal; failures unwind with `Origin`
  (span from the proto span table).
- `vm/natives/*`: lang-reference section 5 core prelude (arithmetic incl. exact ratio `/`, comparisons, lists, dicts
  (`put`, `get`, `merge` as listed), values, strings/console `print`), fuel-metered iteration and lazy ranges;
  `effects.rs`: `d1`..`d9`, `slot`, `once`, `at` (Fn-masked), `hush`, `stop`, `use-bpm`, `use-cycle`, `use-clock` staging
  `StagedEffect`s (the value bound may be any `Value` here; pattern construction is ME-PATTERN/ME-INTEGRATE). Each native
  registers against its `NativeTable` entry; a registration whose arity or mask differs from the entry panics in a test,
  never at run time.

## Required Tests

- `vm/tests/forcing.rs` (TASK-005 forcing criterion, each case separately named): ignored effectful thunk runs exactly
  once; doubly-read `Value` parameter forces once; a wrapper forwarding its parameter TWICE to a numeric consumer runs a
  counter thunk exactly once; a CONDITIONAL wrapper whose forward is skipped still forces once; boundary effect order is
  left to right; a wrapper forwarding to `at` passes the thunk unevaluated; a `Late`-masked domain parameter keeps its
  thunk (use a test native with a `Late` mask standing in for `osc`; the same case through the REAL `osc` native is
  ME-INTEGRATE's `integrate_tex.rs`, which is the evidence for the vactrol-core.md forcing criterion); a fan-out forward with destinations `Value` vs `Fn`
  forces once (the `mixed-forcing` flag is returned by `chase`); an `Undetermined` parameter runs at most once; masks are
  identical with diagnostics off (compile never calls `check`); redefining the directly called callee between `Value`
  and `Fn` switches at the next call; redefining only a DOWNSTREAM callee (`at` among `Value`/`Fn`/`Late` via test
  natives) switches the unchanged wrapper at its next call.
- `vm/tests/query_mode.rs`: `upd` on a global inside a queried closure fails `effect-in-query` with origin; frame-local
  `var` works; `print` is captured, not staged; the scope guard restores Normal after an unwinding failure.
- `vm/tests/failures.rs`: `/ 1 0` (`division-by-zero`), `+ 1 "a"` (`type`), no matching clause (`no-match`), `for x 0..:`
  (`fuel-exhausted`), deep recursion (`depth-exceeded`), calling a number (`not-callable`), wrong arity (`arity`),
  `upd` of a let and of `sound-kit` (`upd-immutable`); each reports an origin span and the next evaluation still works.
- `ns/tests/namespace.rs`: session binding shadows a prelude native at runtime; forms compiled before the shadowing
  binding keep the prelude slot; the prelude value is unchanged after the session binding; fn-local shadowing of a
  session name works; `VarRef` demand: `var cutoff 800`, a closure reading it, `upd cutoff 400` -> the next call sees 400;
  a top-level `let` is a snapshot; re-running a `let` replaces the slot for later evaluations.
- `ns/tests/pkg.rs`: `pads.warm` compiles through `PkgNs`; lookup order locals > session > opens (most recent wins) >
  prelude; re-import replaces the `PkgNs`.
- `ns/tests/stage.rs`: effects released only on success; a bind-then-fail form releases nothing (`RecordingSink` empty).
- `vm/tests/tweak.rs`: a pattern-literal site in a top-level form gets a `TweakId` stamped with its `FormGen`; a
  `Direct`-tier write is observable at the next read without re-evaluation; tiers classified per design 13.
- `vm/tests/natives.rs`: each native registered by this wave matches its table entry (arity, keywords, mask); the
  lang-reference section 5 `# => v` examples for core natives evaluate to their annotated print (6.5.3 print).

## Invariants

- The VM never panics on untrusted input; limits come from 7.1.5.
- `Value` stays `!Send`; no threads; no `std::{thread,fs,time,net,process}`.
- `types::masks` is the only mask logic; the VM calls `chase`, never re-implements it.
- Every file under 800 lines. Split `vm.rs` (dispatch loop) and `call.rs` (boundary forcing) as listed; if a natives
  file nears 800, split by name group and record it.
- `src/value/value.rs` is not edited by this plan (the shells are replaced in their own modules; paths stay the same).

## Edit Protocol

Identical to ME-MASKS "Edit Protocol (common to every ME plan)", evidence under
`tmp/me-middle-20260925-s175/ME-VM/attempt-<n>/`. Parallel with ME-CHECK and ME-PATTERN in one working directory:
never edit their files; record and wait out their transient build breaks.

## Verification

The ME-MASKS table and log rule with `<plan>` = `vm`: V1, V2, V3, V3t, V3f, V6a, V6b, V4 (under 800), V5, V7, V8.

## Completion Criteria (map to vactrol-core.md TASK-005)

- [x] Namespace with read-only prelude, session scope, child scopes and the 5.6/5.7 lookup order; `upd-immutable` on non-vars
- [x] Compiler (match via pattern ops, keyword args, splats, masks from `types/masks.rs`, path/url constants, `ListProv`, depth guard)
- [x] VM (frames, fuel, depth/re-entry limits, boundary forcing with `chase`, memo cells, Query mode, origin unwind)
- [x] All forcing-mask tests listed above pass (TASK-005 forcing criterion; the real-`osc` case is ME-INTEGRATE's `integrate_tex.rs`)
- [x] Package namespaces: `pads.warm` through `PkgNs`, lookup order, re-import replacement
- [x] Query effect mode tests pass
- [x] Failure cases unwind with origin and leave the session usable
- [x] Redefinition and `upd` observable through `VarRef` demand; `Direct`-tier tweak writes observable without re-eval
- [x] Staged effects released only on success
- [x] V1-V8 pass with logs cited; `final-hashes.txt` written

## Progress Log

### Session 177 (2026-09-25), attempt 1: implemented

**Work done**
- `ns/`: `namespace.rs` (`VarSlot`/`VarSlotRef` with a process-unique id, `SlotKind {Let, Var, Fn, Tweak, Prelude}`,
  `Prelude` with `register`/`register_custom`/`register_value`, `Namespace` with the 5.6 scope chain and the 5.7 lookup
  order, `reserve` for forward references, `define`/`redefine`/`write_var`, `import`, `check_env`, `callee_mask` for the
  `Forward` chase), `pkg.rs` (`PackageId`, `PkgNs`, `ImportBinding`), `tweak.rs` (`TweakSite`, `SiteTier`,
  `SiteOrigin`, `TweakTable::write`), `stage.rs` (`SlotKey`, `StagedEffect`, `EffectBuffer`, `EffectSink`,
  `RecordingSink`).
- `compile/`: `compiler.rs` (`CompileCx`, `compile`, name resolution, expressions, calls, blocks, thunks, closures),
  `matchc.rs` (match to pattern ops, or-patterns, guards, list/dict/variant/struct patterns, destructuring, lambdas,
  `fn`/`inst`/`look`, `enum`/`struct` constructors), `proto.rs` (`FnProto`, `Closure` with memo cell, `Arity`, call/list
  sites, `Shape`, the function builder and header parsing), `sites.rs` (tweak-site tiers; `let`/`var`/`upd`).
- `vm/`: `ops.rs`, `vm.rs` (dispatch loop, run, fuel, Query mode guard, `ReadObserver`), `frame.rs` (frames, pending
  calls, forcing, slot reads), `call.rs` (boundary forcing with `masks::chase`, memo cells, argument binding, natives,
  re-entry, `NativeCx`), `natives/{mod,num,list,dict,value,console,effects}.rs` (66 core natives).
- Tests (82 new): `vm/tests/{forcing,query_mode,failures,natives,tweak}.rs`, `ns/tests/{namespace,pkg,stage}.rs`,
  `compile/tests/{compile,sites}.rs`, shared `Sess` harness in `vm/tests/mod.rs` (read -> expand -> compile -> run).

**Design differences (the design wins; recorded here)**
- Ops: `Call(u8)` for positional calls; `CallKw(site)` takes a call-site index (the positional/pair/splat layout, which a
  splat makes dynamic) instead of `(u8, u8)`; `MakeList(site)` takes a list-site index (layout plus `ListProv`). Added
  `LoadCapture`, `DefGlobal`, `UpdGlobal`, `UpdCell` (the plan's `UpdLocal`, generalized so a captured local `var` can be
  updated), `MakeCell` (a local `var` is a local `VarSlot` cell), `MakeShape`, `TestShape`, `TestKey`, `TestLenMin`,
  `TestTruthy`, `GetKey`, `Dup`. `BindField` reads a sequence item; variant fields are read by name (`GetKey`) because
  `VariantVal` fields are key-sorted.
- `Arity` is `{fixed, names, keys}`; keyword-parameter defaults are expressions evaluated where the `fn` is defined and
  stored after the captures in `Closure::captures` (design 8.1 sketches `keys: [(KwId, Value)]`). A positional
  parameter may also be passed by name (the constructor rule).
- `VarSlot.kind` is a `Cell` (Live redefinition may turn a `let` into a `var`); `SlotKind` adds `Prelude`. `Namespace`
  methods take `&self` (interior mutability) because `DefGlobal` defines slots while a proto runs against `&Namespace`.
- A name that resolves nowhere at compile time reserves an unbound session slot (forward references and recursion);
  loading it before definition is `undefined-name`. A reservation never hides a prelude or open-import name.
- A bare name on a line of its own that names a zero-parameter native is a call (`hush`, design-music section 1). A
  bare user `fn` name stays a value; zero-parameter user functions are not specified (recorded as a residual question).
- A top-level form's result is `Deref`ed (a `VarRef` result shows its value); thunks are not forced there.
- `/` of two ints is exact: an integral quotient is an int, otherwise a ratio; any zero divisor is `division-by-zero`.
- 7.1.5 hardening: `Vm::stack_budget` and `CompileCx::stack_budget` (768 KiB default) bound Rust stack use; a nested
  re-entry past it is `depth-exceeded` and a form past it is `nesting-too-deep`, so neither deep re-entry nor deep nesting
  can overflow a 2 MiB test thread or the 1 MiB wasm32 stack (debug builds reach it before 64 re-entries or 1024 levels).
- `import` nodes compile to `nil` (package loading is TASK-009); `load` is not registered here (ME-REACTIVE).
- `lang-reference.md` annotations that disagree with their own bindings are asserted with the computed value and noted in
  `vm/tests/natives.rs`: `map arr {x -> * x 2}` is `[24 24 88]` for `arr = [12 12 44]`, and `put d gain: 1.0 pan: 0`
  keeps `amp: 0.5`.
- A wrapper forwarding to a test native registered with `register_custom` (not in `NativeTable`) infers `Undetermined`
  (types/masks.rs resolves natives through the table); the forcing tests call such natives directly.

**Seams for later waves**: `ReadObserver` (eager-read recording and dirty-read abort for ME-REACTIVE; tested);
`Vm::{run, call_value, force, with_effect_mode, set_fuel, take_output, effects_mut, set_host, take_host}` (`set_host` carries the ME-REACTIVE `SourceLoader` to the `load` native, which cannot capture state); `EffectBuffer::{release,
truncate, take}`; `Prelude::{register, register_custom, register_value}`; `Namespace::{session_value, check_env,
callee_mask, tweaks}`; `TweakTable::{sites_of, set_tier, retire, write}`; `VarSlotRef::{restore, owner, id}`.

**Evidence** (shared tree, after the last source change; logs under `target/fe-logs/`)
| Row | Log / command | Result |
|-----|---------------|--------|
| V1 | `vm-build-s177-2.log` | exit=0, no warnings |
| V2 | `vm-clippy-s177-2.log` | exit=0 |
| V3 | `vm-nextest-s177-2.log` | exit=0, 409 run, 409 passed |
| V3t | `vm-cargotest-s177-2.log` | exit=0, 401 + 8 passed (lib + spec_fixtures), 0 failed |
| V3f | `vm-fixtures-s177-2.log` | exit=0, 8 run, 8 passed |
| V6a | `vm-wasm32-s177-2.log` | exit=0 |
| V6b | `vm-wasm32-hostwasm-s177-2.log` | exit=0 |
| V4 | `find src tests -name '*.rs' -exec wc -l {} + \| sort -n \| tail -5` | largest `src/compile/compiler.rs` 792 (< 800) |
| V5 | `grep -rnE 'std::(thread\|fs\|time\|net\|process)' src/` | `none` |
| V7 | `git diff --stat -- Cargo.toml Cargo.lock` | empty |
| V8 | `rustfmt --edition 2021 --check` on the 35 owned `.rs` files | exit 0 |

Hashes and intents: `tmp/me-middle-20260925-s175/ME-VM/attempt-1/{pre-edit-hashes.txt, intent.md,
post-edit-hashes.txt, final-hashes.txt}`. While ME-CHECK's `src/types/infer_call.rs` did not compile mid-session, the
iteration builds ran in a private snapshot (`scratch.sh` in the same directory; no shared file outside ME-VM paths was
edited); the table above is from the shared tree once it built again.

The `-1` logs (same rows, all exit=0: nextest 400/400, cargo test 392 + 8) predate the self-review fixes below; the
`-2` logs are the final-source evidence.

**Author self-review fixes (independent read-only review, same session)**
- A top-level `let [a] [x]` stored the list item's `VarRef` unforced, so the binding was late instead of a snapshot and
  `var [y] [y]` could make a slot refer to itself, which looped with no fuel check: destructured values are now forced,
  and every slot read (`read_slot`) costs one unit of fuel, so no `VarRef` chain can loop unbounded; `Vm::force` is a
  loop instead of Rust recursion.
- Proto table indices (constants, list sites, nested protos, shapes) saturated at `u16::MAX` and silently read the
  wrong entry past 65536: they now fail the form with `nesting-too-deep` (the closest listed code; "the form is too
  large to compile").
- Query mode let a closure update a local `var` cell it captured from outside the query (state that outlives it): a
  cell created before the query is now `effect-in-query`; cells created inside the query stay writable (10.4).
- `var [a b] ..` inside a body bound read-only locals: they are now `var` cells.
- `min`/`max` failed on `VarRef` list items: they now read items through `NativeCx::deep`.
- Not changed: a local `fn` cannot call itself (lang-reference section 3 decides "no local recursion; lift to a
  top-level fn"; the name resolves outside the body). Dropping or printing a value nested ~100k deep recurses in the
  value module (TASK-001, not an ME-VM path): recorded as a residual risk.

**Blockers**: none.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-005)
- **Previous**: vactrol-middle-masks.md, vactrol-middle-frontend.md
- **Next**: vactrol-middle-reactive.md (ME-REACTIVE)
