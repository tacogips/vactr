# Vactrol Back End: Instruments, Ugen Values, Buses, Templates (BE-INST) Implementation Plan

**planId**: BE-INST (vactrol-core.md TASK-008 language side: `inst` evaluation to `InstDef`, templates, effects in three positions, `bus`/`master`)
**Status**: Completed (accepted by integration review; reconciled by BE-FINAL session 186; archive after the workflow commit)
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

- [x] `inst` evaluates to `InstDef` over the extended catalog; templates ship as prelude `.vact` source
- [x] Effects usable in all three positions; `bus`/`master` lower to `BusDef`; `osc` sound; resolution order
- [x] Implicit control names and the B2 overload set implemented as recommended
- [x] The criterion-2 half "compiles from prelude source to an `InstDef`; template parameters are controls" proven here
      (rendering is BE-FINAL's)
- [ ] V1-V8, I1, I2 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-INST implementer)` entry: work done, fixture multiset changes,
design differences, hash/intent paths, evidence per row, blockers. Edit only this log.)

### Session: 2026-09-25 (session 182, BE-INST implementer)

**Work done** (all edits inside BE-INST writePaths; intent, pre/post hashes and the repair request under
`tmp/be-backend-20260925-s181/BE-INST/attempt-1/`: `intent.md`, `pre-edit-hashes.txt`, `post-edit-hashes.txt`,
`final-hashes.txt`, `repair-request-types-tests-natives.md`):
- `src/dsp/build.rs`: `lower_inst`/`lower_bus` (post-order nodes, output last, Rc dedupe, one `Param` node per control,
  port encoding documented in the module doc, list args -> `node_params`, signals -> synthetic param
  `SIGNAL_CTL_BASE + k` with `Ctl::Cell` + `SignalInput`, `NODE_CAP` -> `graph-too-large`, constant grain limits ->
  `CapabilitySet::require` diagnostics), `UGENS`, `ports`, `effect_ports`, `param_id`, `EXTRA_PARAMS`, `DSP_KEYWORDS`.
- `src/ns/insts.rs`: `InstRegistry` (names, insts, buses, master = `BusId(0)`, cell range 896..1024, custom header
  ctl ids from 192, caps), `realize_inst` (closure called once with `Param` nodes under the VM inst-body flag; tweak
  defaults -> `Ctl::Cell` with `default_cells`, else encoded `Ctl::Const`; list defaults stay node constants), rebinding
  to `Sound::Inst(id)` + staged `Install`, `inst-failed` on any failure, `load_templates`, `impl InstResolver` (for
  the registry and `Rc<RefCell<..>>`: `Sound::Inst`/registry keyword -> `Audio`, other builtin -> `sampler` +
  `SampleSrc::Bank`, sample file -> `sampler` + `SampleSrc::Path`, MIDI, OSC, else `unknown-sound`).
- `src/vm/natives/dsp.rs`: ugen/effect natives, `bus` (routing control with a pattern subject, definition with a
  keyword + block), `master`, B2 collision dispatch (`collides`/`collision`, called by `vm/call.rs` before the arity
  check), `saw`/`tri` signal callee -> ugen inside a body, `+ - *` node building (`arith`, called by `num.rs`),
  `{range sine 0 1}` -> `Sig::MapRange` signal input, `lowpass`/`highpass`/`bandpass` values.
- `src/vm/vm.rs` (`dsp: DspCx`), `src/vm/call.rs` (two hooks), `src/vm/natives/{mod,num,tex}.rs` (registration, ugen
  arithmetic, `osc "/addr"` -> `Sound::Osc`), `src/vm/query_vm.rs` + `src/pattern/eval.rs` (`QueryVm::inst_sound`,
  default `None`) + `src/pattern/combinators/sound.rs` (kit miss -> registry -> `unknown-sound`) +
  `src/pattern/tests/stub_vm.rs`.
- `src/compile/{compiler,matchc}.rs`: `dsp` depth + call-head flag; free control-table names in value position inside
  `inst`/`bus`/`master` bodies (unbound or prelude-only) compile to `Param` constants; an `inst` body does not resolve
  its own name (so `inst additive ..: additive freq ..` calls the ugen); `literal`/`binder_name` moved to `matchc.rs`
  and re-exported (compiler.rs stays at 775 lines).
- `src/ns/evaluator.rs`: `Evaluator::with_insts` (shared registry for the runtime), `insts()`, templates realized at
  construction with their `Install` effects released to the sink, realization in `attempt` with the gate armed (the
  body's reads are the form's edges; a rebuilt inst re-installs), realization diagnostics attached to the form.
- `src/ns/namespace.rs`: a realized inst slot reports `BindKind::Inst` (session inst names are known sounds).
- `src/types/*`: `Ty::UGen` + `ugen` notation, ugen input coercion in `unify.rs`, 112 ugen/effect entries + `bus`,
  `master`, svf modes in `natives_domain.rs`, `dsp_call` rule in `infer_call.rs` (ugen operand of `+ - *`, ugen
  subject of a shared name, `osc` on a string), `bus :k {block}`/`master {block}` definition heads (muted like inst)
  in `check.rs`/`infer.rs`, session inst names accepted by the sound-keyword check in `manifest.rs`.
- `src/prelude/templates.vact`: `sampler`, `analog`, `fm`, `pd`, `additive`, `wavetable`, `granular` (plain header form).
- Tests: `src/types/tests/inst.rs` + `inst/{instdef,implicit,overloads,ugens,effects,capability,buses,resolve,
  templates}.rs`, `src/compile/tests/inst.rs`, `src/vm/tests/inst.rs`, `src/ns/tests/inst.rs` (58 tests).

**Fixture multiset changes (I2)**: none. `git diff -- tests/fixtures/spec/manifest.toml` is empty
(`target/fe-logs/be-inst-manifestdiff-s182-1.log`, exit=0); the spec fixtures pass unchanged
(`target/fe-logs/be-inst-fixtures-s182-1.log`: 10 run, 10 passed, 1 skipped, exit=0).

**Design differences (the design wins; recorded for BE-FINAL)**:
1. Template names are registry sounds (`s :analog`), not prelude bindings: `additive`, `wavetable` and `granular` must
   stay the ugens in scope.
2. Cells for cell-backed inst defaults and signal inputs come from the registry range 896..1024 (`INST_CELL_BASE`);
   the runtime initializes them from `InstRegistry::default_cells(id)` (tweak slots) and `signal_inputs()`. The bus
   control's keyword -> `BusId` lookup is `InstRegistry::bus(kw)` (not part of `InstResolver`).
3. The design-music `sampler` example's `{pitch-to-rate note}` has no catalog node; the template uses `rate: speed`.
4. A pattern as a bus parameter (design-music section 6 `freeze: {alt false true}`) fails the bus definition (`type`
   wrapped as `inst-failed`); a control name on a bus starts the unit from the control's default.
5. The inline forms `inst pd: ...`, `inst organ: ...` and `inst drum: sampler ...:` read with the pipe applied to the
   whole form (B3 shape) and stay `type` failures; the tests realize their header-form equivalents.
6. Any failure inside a `bus`/`master` body is `inst-failed`; `graph-too-large` rides as a diagnostic.

**Verification (session 182, shared tree; logs under `target/fe-logs/`)**:
| Row | Log | Result |
|-----|-----|--------|
| V1 build | be-inst-build-s182-1.log | exit=0 |
| V2 clippy -D warnings | be-inst-clippy-s182-1.log | exit=101; findings only in BE-DSP/BE-SCHED in-progress files (dsp/effects/*, dsp/ring.rs, dsp/ugen/mod.rs, sched/runtime.rs, sched/telemetry.rs, sched/tests/*); none in BE-INST files |
| V3 nextest | be-inst-nextest-s182-2.log (fail-fast) / -3.log (--no-fail-fast) | exit=100; 623 run, 622 passed, 1 failed: `types::tests::natives::required_names_present_and_out_of_scope_absent` (BLOCKER below); -1.log is an invalid run (shell quoting) |
| V3t cargo test | be-inst-cargotest-s182-1.log | exit=101; lib 606 passed, 1 failed (same test) |
| V3f fixtures | be-inst-fixtures-s182-1.log | exit=0; 10 passed, 1 skipped |
| I1 own | be-inst-own-s182-1.log | exit=0; 58 passed |
| V6a wasm32 | be-inst-wasm32-s182-1.log | exit=0 |
| V6b wasm32 host-wasm | be-inst-wasm32-hostwasm-s182-1.log | exit=101 in BE-DSP's `src/dsp/engine.rs:402` (`BusGraph::retire` missing, in progress); the same build with BE-INST's final files over an earlier DSP state exited 0 (isolated copy, Rust 1.83) |
| V4 file sizes | be-inst-wc-s182-1.log | every BE-INST file < 800 (max 777 `ns/evaluator.rs`); `dsp/engine.rs` 864 is BE-DSP's |
| V5 std grep | be-inst-stdgrep-s182-1.log | none |
| V8 git status | be-inst-gitstatus-s182-1.log | exit=0 |
| I2 manifest diff | be-inst-manifestdiff-s182-1.log | empty |
| fmt (owned files) | rustfmt --check on every BE-INST .rs file | exit=0 |

**Blocker**: `src/types/tests/natives.rs` (owned by no plan: not in BE-INST writePaths, not in BE-FINAL sharedPaths)
asserts that `bus`, `master`, `granular`, `granulate`, `sin-osc`, `env-perc`, `compressor`, `vco` are absent from the
native table ("TASK-008, so they stay undefined-name"). This plan's required deliverable adds exactly those entries,
so the assertion is obsolete by construction; it is the only failing test. Repair (one hunk, recorded in
`repair-request-types-tests-natives.md`): move the eight names to the present list, keep `struct`, `inst`, `look`,
`sampler` absent. Resume criterion: the dispatch manifest grants `src/types/tests/natives.rs` to BE-INST (or BE-FINAL
sharedPaths) and the hunk is applied, then V3/V3t rerun green.

**Downstream (not BE-INST's)**: template render tests (BE-FINAL), runtime wiring of `default_cells`/`signal_inputs`/
`bus(kw)` (BE-SCHED/BE-FINAL), `HostManifest` editor metadata and `deferred` fixture reclassification (BE-FINAL).

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-008)
- **Previous**: vactrol-backend-contracts.md
- **Next**: vactrol-backend-native.md, vactrol-backend-wasm.md, vactrol-backend-finalize.md

### Closing note (BE-FINAL, session 186)

Accepted by the integration review (acceptedPlanIds) and reconciled by BE-FINAL on the joined tree: every final-tree gate exits 0 (`target/fe-logs/be-final-<check>-s186-1.log`), and the TASK-007/008 checkboxes in vactrol-core.md cite this plan's tests. BE-FINAL serial repairs in this plan's area: R1 (inst header parameters as pattern controls at run time), R2a/R2b (catalog port wiring, template resource defaults), R5 (`loop-at` bool), R6 (effect-local parameter ids); pinned tests updated in `src/types/tests/inst/{implicit,templates,buses,effects}.rs` and `src/types/tests/diags.rs`. Archive to impl-plans/completed/ in the separate docs commit after the workflow commit.
