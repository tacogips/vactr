# Vactrol Middle End: Namespace, Compiler, Bytecode VM, Tweak Sites, Staging (ME-VM) Implementation Plan

**planId**: ME-VM (implements the non-reactive part of vactrol-core.md TASK-005)
**Status**: Ready
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

- [ ] Namespace with read-only prelude, session scope, child scopes and the 5.6/5.7 lookup order; `upd-immutable` on non-vars
- [ ] Compiler (match via pattern ops, keyword args, splats, masks from `types/masks.rs`, path/url constants, `ListProv`, depth guard)
- [ ] VM (frames, fuel, depth/re-entry limits, boundary forcing with `chase`, memo cells, Query mode, origin unwind)
- [ ] All forcing-mask tests listed above pass (TASK-005 forcing criterion)
- [ ] Package namespaces: `pads.warm` through `PkgNs`, lookup order, re-import replacement
- [ ] Query effect mode tests pass
- [ ] Failure cases unwind with origin and leave the session usable
- [ ] Redefinition and `upd` observable through `VarRef` demand; `Direct`-tier tweak writes observable without re-eval
- [ ] Staged effects released only on success
- [ ] V1-V8 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one entry per session: work done, design differences, hash/intent paths, evidence per row, blockers.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-005)
- **Previous**: vactrol-middle-masks.md, vactrol-middle-frontend.md
- **Next**: vactrol-middle-reactive.md (ME-REACTIVE)
