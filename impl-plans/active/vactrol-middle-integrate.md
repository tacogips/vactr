# Vactrol Middle End: Integration, Domain Natives and Spec Fixture Evaluation (ME-INTEGRATE) Implementation Plan

**planId**: ME-INTEGRATE (completes vactrol-core.md TASK-004/005/006 criteria that need the whole pipeline)
**Status**: Completed (implemented, gate-verified, adversarial review 0 blocking, integration review accepted in session 179; removed from the dispatch manifest by the session-180 amendment; source rides in the single workflow commit; archiving to impl-plans/completed/ after the workflow commit, on user confirmation)
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
- Serial repairs (session-179 checkpoint amendment; details in
  `tmp/me-middle-20260925-s175/ME-INTEGRATE/attempt-1/repair-requests.md`):
  - `src/value/dict.rs` (R1): `put`, Dict arm: an element that is itself a `Value::Dict` merges its pairs (later
    wins); other elements go through `as_pair` as now (`put d & other` keeps working). Covered by
    `integrate_sound.rs` and the manifest case `eval-sound-kit-override`.
  - `src/compile/matchc.rs` (R2): `shape_of` treats a name as a variant pattern only when it names the variant itself
    (`Value::Variant` whose tag is that name, or a ctor `Fn` whose proto name is that name); otherwise it binds.
  - After both repairs: drop `blocked_by`/`defect_run_fails` from case `eval-sound-kit-override` and lang-reference
    block 4, switch `integrate_sound.rs` back to the verbatim `put default-sound-kit [bd: ..]` spelling, and check the
    two open Completion Criteria with the new logs.

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

- [x] `QueryVm` for `Vm`, domain natives, prelude sound-kit values and the `check` stage in `eval_form`/`load` implemented
- [x] Native-table completeness test passes
- [x] TASK-004: fixtures type-check per their classification; annotated negatives produce their codes in isolated sessions
- [x] TASK-005: all `positive` fixtures produce their annotated values via read -> expand -> check -> compile -> run; `diagnostic` fixtures produce their codes; `authority-question` and `illustrative-excluded` are tracked with dispositions (attempt-2: repair R2 applied, lang-reference block 4 line 157 runs clean, `blocked_by`/`defect_run_fails` removed)
- [x] TASK-006: `PParam::Late`/`PParam::Fn` via the VM, the tweaked-probability re-query in both directions, bind-time input-lane rejection, failing visual chain keeps the previous binding
- [x] TASK-005 forcing criterion, visual case: `osc {* 20 {sin time}}` through the real `osc` native keeps its thunk deferred via the `Late` mask and forces it only at uniform resolution (`integrate_tex.rs`)
- [x] Sound-kit late binding, `kit:` and sound-value cases pass (attempt-2: repair R1 applied; `integrate_sound.rs` uses the verbatim `put default-sound-kit [bd: ..]` spelling and case `eval-sound-kit-override` passes with no `blocked_by`)
- [x] No block has `eval = "unclassified"`; V1-V8 pass with logs cited; `final-hashes.txt` written

## Progress Log

### Session s180 (2026-09-25), attempt-1

**Work done**
- `src/vm/query_vm.rs`: `VmQuery { vm, ns }` implements `QueryVm`: every call runs under `Vm::with_effect_mode(Query)`
  (restored on failure), with a fresh `fuel_limit` budget (1,000,000 by default) that is given back to the caller
  afterwards; `call` goes through `Vm::call_value` (one Rust-level re-entry); `deref` is an eager `read_slot`;
  `sound_kit()` reads the SESSION `sound-kit` binding, else the prelude's.
- Domain natives registered against their table entries: `natives/pattern.rs` (transforms, steps, combinators,
  `segment`, `midi-notes`, plus the shared coercions and the bind-time `reject_input_lanes`), `natives/music.rs`
  (the 24 controls with `n`/`note`, `chord`, `voicing`, `arp`, `range`, and the `scale`/`shape` subject overloads),
  `natives/signal.rs` (signal prelude values, `irand`, `fft`, `cc`, `lag`, `map-range`), `natives/tex.rs` (sources,
  geometry/color, blend and modulate families, `add`/`sub` overloads, `src`, `text`, `out` with `compile_tex`
  validation, render settings, `o0`..`o3`), `natives/sound.rs` (`s`/`sound` with `kit:`, `sample`, `midi`,
  `default-sound-kit` and `sound-kit`: one `Sound::Builtin(k)` per `spec_default().sound_kit_keys()` key).
- Shared hunks: `vm/natives/mod.rs` (`register_domain`, idempotent, and `full_prelude`; `register_core` is unchanged
  because the ME-VM test `each_registration_matches_its_table_entry` pins `Prelude::core()` to the core set);
  `vm/mod.rs` (`query_vm` module, `VmQuery` re-export); `ns/evaluator.rs` (`Evaluator::new` registers the domain;
  `eval_form` checks the form against `ns.check_env()` + `HostManifest::spec_default()` before the attempt and prepends
  the diagnostics, never gating; `attempt` rejects a bind whose pattern has an invalid `midi-notes` lane with
  `input-lane-operator` (value `Err(type)`, so the snapshot rolls back and nothing is staged) and drains the loaded
  files' check diagnostics); `ns/load.rs` (`LoaderHost` carries the loaded files' diagnostics, `run_source` checks the
  file as one document in the fresh prelude child scope before running it, `take_load_diags`); `vm/tests/mod.rs`
  (module declarations); `src/lib.rs` (crate doc: reader, expander, checker, namespace/compiler/VM, pattern engine,
  clock, visual chains). `types/manifest.rs`, `types/diag.rs`, `vm/fail.rs` were not edited (no key or code added).
- Tests: `native_table.rs` (4), `integrate_query.rs` (7 + the shared `Ev` harness), `integrate_sound.rs` (13),
  `integrate_pattern.rs` (6), `integrate_tex.rs` (4). `tests/support/eval.rs` + runner tests
  `blocks_evaluate_per_classification`, `cases_evaluate_per_expectation` and the on-demand `evaluation_report`
  (`#[ignore]`).
- Manifest: all 11 blocks classified; 69 evaluation cases added (52 for the lang-reference sections 1-5 `# => v`
  annotations, the annotated negatives `+ 1 "a"` (type-mismatch), `?T` as `T` (optional-as-value), rebinding,
  `upd` of a `let` (upd-immutable), `/ 1 0`, `var` rebinding, `round 90.7`, and the 7.1.7 non-verbatim scope,
  SOUND FIRST and sound-kit cases). `music-chord-seven-conflict` stays a `verbatim = false` misplaced-colon case.

**Fixture classification**

| Block | eval | Reason |
|-------|------|--------|
| lang-reference #1 | positive | both forms check and run clean |
| lang-reference #2 | diagnostic | annotated negatives (upd-immutable@25, rebinding@27/@28), one-document rebinding of `a`/`arr` (@68/@80), fn parameters shadow session `a` (warnings), placeholder `my-kick` |
| lang-reference #3 | diagnostic | illustrative literal listings are calls, free example names, annotated negatives (duplicate-key, unknown-keyword/unknown-field, division by zero, `+ 1 "a"`), pattern names shadow `p` |
| lang-reference #4 | diagnostic, blocked_by R2 | free example names, rebinding@51, shadowing warnings, unbounded `for`; line 157 `no-match` is defect R2 (strict expected failure) |
| lang-reference #5 | deferred TASK-009 (check pinned) | package import and `load` host I/O; pins rebinding@15, unknown-keyword@23, upd-immutable@33, undefined-name@58 |
| design-music #1 | diagnostic | every form runs; `use-clock :link` is clock-source-unavailable@49 (planned) |
| design-music #2 | deferred TASK-008 (check pinned) | `inst pluck` and sound-pack `load`; pins rebinding@32 (M6) and undefined-name@36 (`tr909`); run also hits R1 |
| design-music #3 | diagnostic | placeholder `pat`, the signal listing line, pre-SOUND-FIRST lines 100/103/106 (sound-not-first), subject-less `off {note ..}` / `arp {chord ..}` |
| design-music #4-#6 | deferred TASK-008 | DSP instruments, buses, effects, granular |

No block is `authority-question` or `illustrative-excluded`; the two authority-question CASES stay pending.

**Design differences and findings**
- Spec text errata (implementation follows the Decided rule or the example's own definitions; values pinned with a
  TOML comment per case): lang-reference line 71 `f 1 2 # => 24` is 12 (`* a 12`); line 373 `put d gain: 1.0 pan: 0`
  keeps `amp: 0.5` (`d` is immutable); line 465 `0..8` prints as the range `0..8`; line 671 `map arr ..` is
  `[24 24 88]` with `a = 12`; line 679 `[[0 a] ..]` is `[[0 12] ..]`; lang-reference block 5 `upd base 62` upd's a
  `let` (upd-immutable); design-music block 3 lines 97-106 predate SOUND FIRST (sound-not-first).
- `kit:` given a `var`: the VM forces a native's named arguments at the call boundary (src/vm/call.rs, ME-VM), so the
  `s` native sees the value; a `upd` of the kit is heard at the next query through the reactive pass (the form's
  eager read of the var re-evaluates and rebinds `d1`), not through `PParam::Late`. `s` uses `param_of`, so a late
  keyword argument becomes `PParam::Late` if call.rs ever passes it unforced. Low residual.
- `add`/`sub` on a pattern: `range p k k+1` (exactly `k + v` for exact numbers); a number subject calls `+`/`-`.
- `map-range` supports `s lo hi` only (`Sig::MapRange` has no input range); the 5-argument form is `arity`.
- `render`, `use-fps`, `use-canvas` check the effect mode only: `ns/stage.rs` has no render-settings effect; the
  render host is TASK-007/010.
- Bind-time `input-lane-operator` uses FailCode `type` for the form's failure (7.1.6 has no lane FailCode; the
  diagnostic carries the code).
- `chord` forces a block argument at bind so its `[root quality]` literals are recognized; a `var` stays late.
- The design-visual example `osc {* 20 {sin time}}` with the real core `sin` fails `type` at uniform resolution
  (`sin` does not lift signals); the plan's forcing test uses a counting native, as specified. Follow-up for the
  visual vocabulary owner.
- 11.3 staging invalidation of uncommitted events is TASK-007's; `integrate_pattern.rs` asserts only the re-query
  semantics it relies on.

**Repairs / blockers** (details: tmp/me-middle-20260925-s175/ME-INTEGRATE/attempt-1/repair-requests.md)
- R1: `put` of a dict element must merge (7.1.4, design-music sound kits); src/value/dict.rs `put` fails `type`.
- R2: src/compile/matchc.rs `shape_of` compiles a match name bound in the session to a variant VALUE as a variant
  pattern; the checker binds it (Decided Q1), so lang-reference block 4 line 157 fails `no-match`.
Both are outside ME-INTEGRATE writePaths and not applied; the manifest marks them with `blocked_by` +
`defect_run_fails` (strict expected failures that break the runner once repaired).

**Evidence** (tmp/me-middle-20260925-s175/ME-INTEGRATE/attempt-1/: pre-edit-hashes.txt, intent.md,
post-edit-hashes.txt, final-hashes.txt, report-final.txt, report-probe.txt, repair-requests.md, v4/v5/v7/v8 files)

| # | Log | Result |
|---|-----|--------|
| V1 | target/fe-logs/integrate-build-s180-1.log | exit=0 |
| V2 | target/fe-logs/integrate-clippy-s180-1.log | exit=0 |
| V3 | target/fe-logs/integrate-nextest-s180-1.log | 489 run, 489 passed, 1 skipped (the ignored report), exit=0 |
| V3t | target/fe-logs/integrate-cargotest-s180-1.log | lib 479 passed, spec_fixtures 10 passed (1 ignored), exit=0 |
| V3f | target/fe-logs/integrate-fixtures-s180-1.log | 10 run, 10 passed (includes the two new evaluation tests), exit=0 |
| V6a | target/fe-logs/integrate-wasm32-s180-1.log | exit=0 |
| V6b | target/fe-logs/integrate-wasm32-hostwasm-s180-1.log | exit=0 |
| V4 | v4-linecount.txt | largest compiler.rs 792; largest owned/shared-edited evaluator.rs 744 |
| V5 | v5-stdio.txt | none |
| V7 | v7-deps.txt | empty |
| V8 | v8-rustfmt.txt | exit=0 on the 14 owned .rs files |
| unclassified | v-unclassified.txt | 0 |

### Session s181 (2026-09-25), attempt-2 (serial repairs)

**Work done** (intents and hashes: tmp/me-middle-20260925-s175/ME-INTEGRATE/attempt-2/intent.md,
pre-edit-hashes.txt, post-edit-hashes.txt, final-hashes.txt)
- R1 `src/value/dict.rs` `put`, Dict arm: an element that is itself a `Value::Dict` merges its pairs (later keys
  win); other elements go through `as_pair` as before, so `put d & other` keeps working.
- R2 `src/compile/matchc.rs` `shape_of`: a global name is a variant/struct pattern only when it names the variant
  itself (`Value::Variant` whose tag is the name, or a ctor `Fn` whose proto name is the name); any other name binds,
  matching the checker's rule (Decided Q1).
- `tests/fixtures/spec/manifest.toml`: `blocked_by`/`defect_run_fails` removed from lang-reference block 4 and case
  `eval-sound-kit-override`; the block 4 and design-music block 2 notes updated. The strict runner passes with the
  pins unchanged otherwise (block 4 no longer fails `no-match@157`; the case's `query_values` hear the 909 sample).
- `src/vm/tests/integrate_sound.rs`: the late-binding test uses the verbatim design-music spelling.
- The first build of attempt-2 failed on a missing `name_of_sym` import in matchc.rs
  (target/fe-logs/integrate-build-s181-2a-failed.log, integrate-nextest-s181-2a-failed.log, exit=101); fixed and
  re-run below.

**Evidence** (tmp/me-middle-20260925-s175/ME-INTEGRATE/attempt-2/)

| # | Log | Result |
|---|-----|--------|
| V1 | target/fe-logs/integrate-build-s181-2.log | exit=0 |
| V2 | target/fe-logs/integrate-clippy-s181-2.log | exit=0 |
| V3 | target/fe-logs/integrate-nextest-s181-2.log | 489 run, 489 passed, 1 skipped, exit=0 |
| V3t | target/fe-logs/integrate-cargotest-s181-2.log | lib 479 passed, spec_fixtures 10 passed (1 ignored), exit=0 |
| V3f | target/fe-logs/integrate-fixtures-s181-2.log | 10 run, 10 passed, exit=0 |
| V6a | target/fe-logs/integrate-wasm32-s181-2.log | exit=0 |
| V6b | target/fe-logs/integrate-wasm32-hostwasm-s181-2.log | exit=0 |
| V4 | v4-linecount.txt | largest compiler.rs 792 |
| V5 | v5-stdio.txt | none |
| V7 | v7-deps.txt | empty |
| V8 | v8-rustfmt.txt | exit=0 on the 14 owned .rs files plus dict.rs and matchc.rs |
| unclassified | v-unclassified.txt | 0 |

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
