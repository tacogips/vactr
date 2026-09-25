# Vactrol Middle End: Integration, Domain Natives and Spec Fixture Evaluation (ME-INTEGRATE) Implementation Plan

**planId**: ME-INTEGRATE (completes vactrol-core.md TASK-004/005/006 criteria that need the whole pipeline)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md sections 7.1.1 (pipeline), 7.1.3 (Query VM handle, native table completeness, source loading), 7.1.4 (sounds and the sound kit, `kit:`), 7.1.7 (INTEGRATE wave, spec fixture evaluation, cases), 10.1, 10.4, 11.7; lang-reference.md sections 1-5 (`# => v` annotations)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/2
**dependsOn**: ME-CHECK, ME-REACTIVE, ME-PATTERN (and transitively ME-MASKS, ME-FRONTEND, ME-VM)
**Dispatch manifest**: impl-plans/active/me-middle-20260925-s175-dispatch.json

---

## Intent and Context

The earlier waves built each layer against stubs. This plan joins them:
- `QueryVm` for `Vm` (`vm/query_vm.rs`): enters Query effect mode with a scope guard on every call, fuel 1,000,000 per
  call, each call counts as a Rust-level re-entry, and `sound_kit()` returns the session `sound-kit` binding when present,
  else the prelude's (7.1.3).
- Domain native wrappers over the ME-PATTERN builders: pattern, control, signal, music, visual and sound natives,
  including `s`/`sound` (one positional argument, optional `kit:`), `sample` (`Sound::Sample(path)`, no I/O) and `midi`
  (`Sound::MidiOut`, channel 1..16), and the runtime overload dispatch of `scale`/`shape` on the subject tag (a tag
  outside the group is `Failure(type)`).
- Prelude values `default-sound-kit` and `sound-kit` (the same dict: one `Sound::Builtin(k)` per `HostManifest::spec_default().sounds` key).
- The checker inserted into the pipeline: `Evaluator::eval_form` and `load` run read -> expand -> check -> compile -> run;
  check diagnostics never gate compile or run (7.1.1), and a form with a reader/expander error is not checked or run.
- The native-table completeness test and the spec-fixture evaluation stages (7.1.7).
- `src/lib.rs` crate doc updated to describe the reader, expander, checker, namespace/compiler/VM, pattern engine, clock
  and visual chains (carried TODO: mention the expander).

## Non-Goals

- No new language behavior beyond the accepted design; no scheduler, staging invalidation machinery, DSP, hosts, file
  I/O, package loading (TASK-007..009).
- No change to settled fixture classifications for reader/expander (`reader`, `expand` keys) except where a design
  change already required it (none expected).
- Authority-question fixtures stay pending (6.5.6); do not pick an answer.

## writePaths (exclusive)

- `src/vm/query_vm.rs`, `src/vm/natives/pattern.rs`, `src/vm/natives/signal.rs`, `src/vm/natives/tex.rs`,
  `src/vm/natives/music.rs`, `src/vm/natives/sound.rs`
- `src/vm/tests/integrate_query.rs`, `src/vm/tests/integrate_sound.rs`, `src/vm/tests/integrate_pattern.rs`,
  `src/vm/tests/integrate_tex.rs`, `src/vm/tests/native_table.rs`
- `tests/spec_fixtures.rs`, `tests/support/mod.rs`, `tests/support/eval.rs` (new), `tests/fixtures/spec/manifest.toml`
- `impl-plans/active/vactrol-middle-integrate.md`

## sharedPaths (pre/post hash each; minimal hunks)

- `src/vm/mod.rs`, `src/vm/natives/mod.rs`, `src/vm/tests/mod.rs`: declare modules; register the domain natives.
- `src/ns/evaluator.rs`: insert the `check` stage (build `CheckEnv` from the session namespace), register prelude
  values, collect check diagnostics into `FormOutcome.diags`; run the bind-time input-lane walk for a slot bind whose
  value is a pattern containing `MidiNotes` and reject the bind with `input-lane-operator` (nothing staged, old binding kept).
- `src/ns/load.rs`: insert the `check` stage for loaded forms (diagnostics reported with their own spans; they do not
  fail the load).
- `src/types/manifest.rs`: add sound keys to `spec_default()` only if a positive fixture needs one (record each key).
- `src/types/diag.rs` / `src/vm/fail.rs`: only to add a code a wave recorded as missing (7.1.3); record it.
- `src/lib.rs`: crate doc comment only.

## Required Tests

- `native_table.rs`: every `NativeTable` entry has exactly one registered implementation (or prelude value) with the
  same arity, keywords and mask, and every registered native has an entry (7.1.3).
- `integrate_query.rs`: `PParam::Late` re-reads a `var` per query (`upd` between two queries changes events);
  `PParam::Fn` calls back into the VM in Query mode (a global `upd` inside it fails `effect-in-query`, `print` captured
  into `QueryResult.output`); fuel and re-entry limits apply per `QueryVm` call.
- `integrate_pattern.rs`: a tweaked `PParam` probability changes the re-queried events in both directions: `maybe` 0 -> 1
  restores previously absent events, `degrade-by` 0 -> 1 removes events (the 11.3 staging invalidation that re-queries
  uncommitted events is TASK-007's; this asserts the re-query semantics it relies on, and the progress log says so);
  `s :pluck > midi-notes channel: 1 > fast 2 > d1` through `eval_form` gives `input-lane-operator`, stages nothing, and
  the previous `d1` binding stays.
- `integrate_sound.rs`: `let sound-kit put default-sound-kit [bd: {sample ./bd/909.wav}]` makes the NEXT query of an
  already-bound `s :bd > d1` resolve to `Sound::Sample(./bd/909.wav)` (late binding heard at the next event);
  `let tr909 [bd909: {sample ./tr909/bd.wav} sd909: {sample ./tr909/sd.wav}]` then `s [:bd909 :sd909] kit: tr909 > d1`
  checks clean and its events carry the two tr909 samples; a `var` kit changed with `upd` is read per query;
  `let kick sample ./kick.wav` then `s kick > d1` carries that sample with no kit lookup;
  `s :bd909 kit: [bd: {sample ./bd.wav}] > d1` checks clean and its query records an event-local `unknown-sound`;
  `s {midi 1} > note [:c :e :g] > d1` events carry `Sound::MidiOut(1)`; `n [0 3] > s :bd > d1` gives `sound-not-first`
  from `check` and a runtime `Failure(arity)` from `s`; `scale`/`shape` dispatch on pattern vs texture/number subjects.
- `integrate_tex.rs`: a failing visual chain bind stages nothing and the previous texture binding stays; TASK-005
  forcing case "a time-varying visual argument stays deferred via its `Late` mask": evaluate
  `osc {* 20 {sin time}} > out o0` through the REAL registered `osc` native with a counting test native standing in
  for `sin` inside the thunk (or a counter `var` read by the thunk); assert the thunk is NOT forced at the `osc` call
  boundary (counter 0 after `eval_form`, the `VParam` holds the thunk), that `resolve_uniforms` at two different frame
  times forces it once per resolution and yields two different uniform values, and that the registered `osc` mask equals
  its native-table entry (all `Late`).
- Spec fixtures (`tests/spec_fixtures.rs` + `tests/support/eval.rs`):
  - Replace `eval = "unclassified"` on every `[[block]]` with one of `positive`, `diagnostic`, `authority-question`,
    `illustrative-excluded`, `deferred` (+ `deferred_to = "TASK-00N"`) per 7.1.7, with a `note` saying why.
  - `positive`: every form checks with no error diagnostic, compiles and runs without failure in one isolated `Evaluator`
    (`NoopHost`, `RecordingSink`). `diagnostic`: exact multisets `check_diags` and `run_fails` as `code@line` (errors and
    warnings only; never hints). `deferred`: no-panic/no-abort only, except blocks deferred ONLY for host file I/O, which
    also pin `check_diags`. Each block is checked as one document (all forms as one session scope) before running form by
    form.
  - Add a `[[case]]` for every `# => v` annotation in lang-reference sections 1-5 with `value` (canonical print) or `fail`
    (a `FailCode`) plus `check_diags`, evaluated in a fresh `Evaluator`; the annotated negatives carry their codes
    (`type-mismatch`, `optional-as-value`, `rebinding`, `upd-immutable`).
  - Add the non-verbatim cases of 7.1.7: `let a 1` twice (`rebinding`), a `fn` parameter shadowing a session name
    (`shadowing`), `n [0 3] > s :bd > d1` and `note [:c] > s :x > d1` (`sound-not-first`), `s :not-a-sound > d1`
    (`unknown-keyword`), `s :bd909 kit: [bd: {sample ./bd.wav}] > d1` (clean check; run records `unknown-sound`), the
    two `kit:` cases and `s kick > d1`, and the sound-kit override pair.
  - New runner tests `blocks_evaluate_per_classification` and `cases_evaluate_per_expectation`; the existing reader,
    expander and verbatim tests stay green; `music-chord-seven-conflict` stays a `verbatim = false` misplaced-colon case.
  - If a fixture shows the implementation disagrees with a Decided spec line, stop and record it; do not reclassify a
    block to hide a failure.

## Invariants

- Check never gates compile/run; the pipeline order is read -> expand -> check -> compile -> run.
- `sound_kit()` reads only the session-level binding; a fn-local `sound-kit` does not affect `s`.
- No `.rs` file reaches 800 lines; no `std::{thread,fs,time,net,process}`; no dependency.
- `tests/spec_fixtures.rs::verbatim_cases_appear_in_their_document` passes.

## Edit Protocol

Identical to ME-MASKS "Edit Protocol (common to every ME plan)", evidence under
`tmp/me-middle-20260925-s175/ME-INTEGRATE/attempt-<n>/`. This wave runs alone (after the join); it is also where
serial repair of cross-wave breakage found at the join is done, each repair recorded with its file, cause and hashes.

## Verification

The ME-MASKS table and log rule with `<plan>` = `integrate`: V1, V2, V3, V3t, V3f, V6a, V6b, V4 (under 800), V5, V7, V8.
V3f must show the new evaluation tests ran. Also record inline `grep -c 'eval = "unclassified"' tests/fixtures/spec/manifest.toml`
(must print `0`).

## Completion Criteria

- [ ] `QueryVm` for `Vm`, domain natives, prelude sound-kit values and the `check` stage in `eval_form`/`load` implemented
- [ ] Native-table completeness test passes
- [ ] TASK-004: fixtures type-check per their classification; annotated negatives produce their codes in isolated sessions
- [ ] TASK-005: all `positive` fixtures produce their annotated values via read -> expand -> check -> compile -> run; `diagnostic` fixtures produce their codes; `authority-question` and `illustrative-excluded` are tracked with dispositions
- [ ] TASK-006: `PParam::Late`/`PParam::Fn` via the VM, the tweaked-probability re-query in both directions, bind-time input-lane rejection, failing visual chain keeps the previous binding
- [ ] TASK-005 forcing criterion, visual case: `osc {* 20 {sin time}}` through the real `osc` native keeps its thunk deferred via the `Late` mask and forces it only at uniform resolution (`integrate_tex.rs`)
- [ ] Sound-kit late binding, `kit:` and sound-value cases pass
- [ ] No block has `eval = "unclassified"`; V1-V8 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one entry per session: work done, fixture classification table (block -> class, reason), design
differences, repairs, hash/intent paths, evidence per row, blockers.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-004, TASK-005, TASK-006)
- **Previous**: vactrol-middle-check.md, vactrol-middle-reactive.md, vactrol-middle-pattern.md
- **Next**: vactrol-middle-finalize.md

### OUTPUT CONTRACT NOTE (operator, 2026-09-25, after the ME-PATTERN attempt-1 failure)

- `planId` belongs ONLY in the step6-implement output payload. The
  step6-test-integrity-check and step7-adversarial-review outputs MUST NOT
  contain `planId` (their contracts reject additional properties; ME-PATTERN
  attempt 1 failed with "output contract $.planId additional property is not
  allowed" after a green gate).
- The adversarial-review output MUST contain the `findings` array (empty when
  none) and the integration-review output MUST contain `needs_revision`.
