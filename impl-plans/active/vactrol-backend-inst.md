# Vactrol Back End: Instruments, Ugen Values, Buses, Templates (BE-INST) Implementation Plan

**planId**: BE-INST (vactrol-core.md TASK-008 language side: `inst` evaluation to `InstDef`, templates, effects in three positions, `bus`/`master`)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 12.8.6 (all), 12.8.7, 12.8.8 (capability gate), 12.1, 12.4, 12.5, 7.1.3, 7.1.4, 13 (inst defaults as Direct sites); design-music.md sections 2, 4, 5, 6; design-docs/user-qa/pending-backend-questions.md B2, B3
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/3
**dependsOn**: BE-CONTRACTS
**Dispatch manifest**: impl-plans/active/be-backend-20260925-s181-dispatch.json

---

## Intent and Context

An `inst` body is evaluated ONCE at definition time on the evaluator thread; ugen functions build nodes, not audio
(12.1). Today `inst` compiles to a closure (`src/compile/compiler.rs` `FnDef::Inst`) and the synthesis/effect/bus
vocabulary is absent from the native table, so it is `undefined-name`. This plan adds the node-building natives, the
`Ty::UGen` typing, implicit control names and the DSP name overloads (B2), realization of an `inst` closure into an
`InstDef` (`dsp/build.rs`), the instrument registry with the `InstResolver` implementation (`ns/insts.rs`), `bus :name:` and
`master:` definitions lowered to `BusDef`, `osc "/addr"` as `Sound::Osc`, the sound-resolution order kit -> registry ->
`unknown-sound`, and the seven prelude templates as `.vact` source. BE-CONTRACTS already declared `Value::UGen`,
`UGenNode`/`UGenKind`/`UGenInput`, `Sound::Inst`, `Sound::Osc`, `StagedEffect::Install`, `InstResolver`, `Route`,
`SignalInput`, `GraphHandle`, the control table and `CapabilitySet`. BE-SCHED and BE-DSP run at the same time.

## Non-Goals

- No audio rendering, no scheduler, no hosts. Template RENDER tests are BE-FINAL's (they need BE-DSP).
- No `inst NAME: TEMPLATE args` derivation (B3): templates use the plain header form; the design-music block keeps
  whatever diagnostics the reader and checker give.
- No user-defined overloads; no new syntax; no changes to reader or expander.
- No `HostManifest` editor-metadata wiring (BE-FINAL). No reclassification of the `deferred` fixture blocks (BE-FINAL).

## writePaths (exclusive; BE-INST is the only writer of these language files after BE-CONTRACTS, design 12.8.2)

- `src/types/ty.rs`, `natives.rs`, `natives_domain.rs`, `check.rs`, `infer.rs`, `infer_call.rs`, `unify.rs`, `scope.rs`,
  `manifest.rs`, `src/types/tests/inst.rs` (+ new files under `src/types/tests/inst/`)
- `src/compile/compiler.rs`, `src/compile/matchc.rs`, `src/compile/tests/inst.rs`
- `src/vm/natives/dsp.rs`, `src/vm/natives/mod.rs` (registration only), `src/vm/natives/num.rs` (`+ - *` ugen overload),
  `src/vm/natives/sound.rs` (`osc` string form, registry-aware resolution), `src/vm/natives/tex.rs` (only to keep `osc`
  numeric = Hydra source when the string overload is added), `src/vm/query_vm.rs`, `src/vm/vm.rs` and `src/vm/call.rs`
  (only if realization needs a VM entry point), `src/vm/tests/inst.rs`
- `src/ns/insts.rs`, `src/ns/evaluator.rs` (realization hook after a definition form commits), `src/ns/namespace.rs`
  (prelude template load), `src/ns/tests/inst.rs`
- `src/value/print.rs`, `src/value/eq.rs` (only if `UGen` printing/equality needs more than the CONTRACTS arms)
- `src/pattern/eval.rs` (`QueryVm::inst_sound`), `src/pattern/combinators/sound.rs` (registry fallback),
  `src/pattern/tests/stub_vm.rs` (stub method)
- `src/dsp/build.rs`, `src/prelude/templates.vact` (new)
- `tests/fixtures/spec/manifest.toml`: ONLY the `check_diags`/`run_fails` lines of NON-deferred blocks whose multisets
  change because a name this plan defines is no longer `undefined-name`; each change recorded (block, old, new, cause)
- `impl-plans/active/vactrol-backend-inst.md`

## sharedPaths

None.

## File-Level Changes (signatures only)

1. `types/ty.rs`: `Ty::UGen`; `unify.rs`: a ugen INPUT position accepts `float | int | ugen | signal` (local coercion,
   like `[sound]` in 7.1.4); `+ - *` get a `ugen` overload in the overload group.
2. `types/natives.rs`/`natives_domain.rs`: one entry per ugen (design-music section 7 sound row), per effect name
   (design-music section 5; named parameters with defaults from `dsp::controls`/the effect's parameter list), `bus`,
   `master`, `osc` string form; the B2 collision set (`saw tri lpf hpf bpf delay comb gain pan room bus range` and every
   effect name that is a control) joins the subject-overload group: pattern subject -> the existing control/signal;
   ugen subject, numeric argument inside an `inst` body, or no subject inside a `bus` body -> ugen/effect; zero-argument
   `saw`/`tri` stay signals.
3. `types/check.rs`/`scope.rs` + `compile/compiler.rs`/`matchc.rs` (12.8.6 "implicit control names", B2): inside an
   `inst`/`bus`/`master` body, a free name that is a `dsp::controls` row and is not bound in scope compiles to a
   `Param(CtlId)` input and types from the row; header parameters stay control names (M3); `bus :name:` + block and
   `master:` + block are definition heads (like `inst`), their block's first element taking the implicit `BusInput`
   subject. `types/manifest.rs`: the synth set includes the seven template names.
4. `vm/natives/dsp.rs`: node-building natives returning `Value::UGen(Rc<UGenNode>)`; an effect call with a pattern subject
   is the SuperDirt control (existing behavior); `bus :name` with a pattern subject stays the routing control.
5. `dsp/build.rs`: `lower_inst(id: InstId, root: &UGenNode, header: &[(CtlId, Ctl)]) -> Result<InstDef, Diagnostic>` and
   `lower_bus(id: BusId, root: &UGenNode) -> Result<BusDef, Diagnostic>`; node count above `NODE_CAP` -> `graph-too-large`;
   list-valued header arguments (`partials`) become node constants; keyword/bool arguments encode through
   `dsp::controls::encode`; `Signal` inputs become control-rate cells (returned as `SignalInput`s); grain and voice caps
   checked with `CapabilitySet::require` when constant.
6. `ns/insts.rs`: `InstRegistry` (`KwId -> (InstId, Arc<InstDef>)`, bus and master defs, signal inputs); realization:
   after an `inst` definition form commits, call the closure ONCE with each header parameter bound to a `Param(CtlId)`
   node (header defaults that are `Direct` tweak sites become `Ctl::Cell` defaults via the site's cell; other defaults
   `Ctl::Const`), lower, register, bind `NAME` to `Value::Sound(Sound::Inst(id))`, and stage
   `StagedEffect::Install(GraphHandle::Inst { .. })`; a failing body is `Failure(inst-failed)` and keeps the previous
   definition. `impl InstResolver for Rc<RefCell<InstRegistry>>`: `route` gives `Audio { sample: None }` for
   `Sound::Inst` and a registry keyword, `Audio { inst: sampler, sample: Some(SampleSrc::Bank { kw: k, index: 0 }) }` (commit fills `index` from `n`) for
   `Sound::Builtin(k)` naming a host bank, `Audio { inst: sampler, sample: Some(SampleSrc::Path(p)) }` for
   `Sound::Sample(p)`; `Midi` for
   `Sound::MidiOut`; `Osc` for `Sound::Osc`; otherwise `Failure(unknown-sound)`. Constructor hands the same registry to
   the `Evaluator` side and to `Runtime::new` (12.8.3).
7. `pattern/eval.rs` + `combinators/sound.rs` + `vm/query_vm.rs`: `QueryVm::inst_sound(&mut self, k: KwId) -> Option<Value>`;
   a keyword missing from the kit in use (from `kit:` or `sound_kit()`) falls back to `inst_sound` before
   `unknown-sound`.
8. `src/prelude/templates.vact`: `sampler`, `analog`, `fm`, `pd`, `additive`, `wavetable`, `granular` in the plain header
   form of design-music sections 4 and 6 (every template parameter a header control), embedded with `include_str!` and
   evaluated into the prelude at construction through the normal read -> expand -> check -> compile -> run path.

## Required Tests (`src/{types,compile,vm,ns}/tests/inst.rs`)

- Each design-music section 2/4/6 `inst` example except the B3 spelling, and each prelude template, realizes to an
  `InstDef` with the expected node kinds and params (`pluck`: saw -> lpf -> mul(env-perc) -> mul(amp)).
- Implicit control names: `attack`/`release`/`amp`/`note` in bodies become `Param` nodes; the same free name outside a
  body is still `undefined-name`.
- Overloads: `lpf 800` on a pattern stays the control; `saw freq > lpf cutoff` in an `inst` builds `Saw -> Lpf`; zero-arg
  `saw` is the signal; `room 0.3` on a pattern stays the control while `plate` in a `bus` builds the effect.
- `bus :drums:` with `compressor > tape > plate` lowers to a three-unit `BusDef`; `master:` lowers; a 257-node body gives
  `graph-too-large`; a failing body gives `inst-failed` and keeps the previous definition.
- Sound resolution: kit first, then registry, then `unknown-sound`; `s :analog` resolves to the template; a builtin bank
  routes to `sampler`; `osc "/x"` is `Sound::Osc` and `osc 20` is still the Hydra source.
- Header `Direct` sites produce `Ctl::Cell` defaults; a signal input produces a `SignalInput`; a staged `Install` effect
  is released only when the form succeeds.
- All existing tests keep passing; changed fixture multisets are listed in the progress log.

## Invariants

- No closure, `Rc` or `Value` crosses into `InstDef`/`BusDef` (they are `Arc`-shareable POD structures).
- The checker never aborts (7.1.1); diagnostics never gate compile or run.
- No `std::{thread,fs,time,net,process}`. No `.rs` file reaches 800 lines.

## Edit Protocol

Common protocol of `vactrol-backend-contracts.md`; evidence under `tmp/be-backend-20260925-s181/BE-INST/attempt-<n>/`.
Runs concurrently with BE-SCHED (which edits `pattern/step.rs` and `pattern/combinators/control.rs`, never the pattern
files above) and BE-DSP.

## Verification

Common table with `<wave>` = `inst`: V1, V2, V3, V3t, V3f, V6a, V6b, V4, V5, V8. Plus I1:
`NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/::inst/)'`
as LOG(`be-inst-own`), and I2: `git diff -- tests/fixtures/spec/manifest.toml` (every hunk explained in the progress log).

## Completion Criteria (map to vactrol-core.md TASK-008)

- [ ] `inst` evaluates to `InstDef` over the extended catalog; templates ship as prelude `.vact` source
- [ ] Effects usable in all three positions; `bus`/`master` lower to `BusDef`; `osc` sound; resolution order
- [ ] Implicit control names and the B2 overload set implemented as recommended
- [ ] The criterion-2 half "compiles from prelude source to an `InstDef`; template parameters are controls" proven here
      (rendering is BE-FINAL's)
- [ ] V1-V8, I1, I2 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-INST implementer)` entry: work done, fixture multiset changes,
design differences, hash/intent paths, evidence per row, blockers. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-008)
- **Previous**: vactrol-backend-contracts.md
- **Next**: vactrol-backend-native.md, vactrol-backend-wasm.md, vactrol-backend-finalize.md
