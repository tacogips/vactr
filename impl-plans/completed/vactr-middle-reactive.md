# Vactr Middle End: Reactive Dependency Graph, Evaluator and load (ME-REACTIVE) Implementation Plan

**planId**: ME-REACTIVE (implements the reactive part of vactr-core.md TASK-005, plus `load` and the `NoopHost` stub)
**Status**: Completed (implemented, gate-verified, adversarial review 0 blocking, integration review accepted in session 178; removed from the dispatch manifest by the session-179 amendment; source rides in the single workflow commit; archiving to impl-plans/completed/ after the workflow commit, on user confirmation)
**Design Reference**: design-docs/specs/design-implementation.md sections 5.6 (revised reactive graph, pass journal, rounds, Failed/Blocked, attempt edge sets, status events, equality cutoff, rebuild eligibility), 7.1.3 (Staged effects, Top-level driver, Source loading), 7.1.5, 13 (reeval tier); lang-reference.md section 4 (`load`)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactr/issues/2
**dependsOn**: ME-VM (namespace, compiler, VM, staging, `d1`..`d9` staging natives)
**Dispatch manifest**: impl-plans/active/me-middle-20260925-s175-dispatch.json

---

## Intent and Context

Changing a value updates every binding computed from it, recomputing only the affected part (design 5.6 revised). This
plan builds `DepGraph` (eager-read edges over all top-level slot kinds via `Deref` instrumentation; `Late` captures give
no eager edge), the pass journal with provisional commits, end-of-pass validation and rollback, pass-level staging of
every host-visible effect, deref-time Failed/Blocked checks with dirty-read abort and retry, recovery subscriptions,
attempt edge sets, status events in both directions, the equality cutoff, whole-form transactions, rebuild eligibility
(partially superseded multi-output forms and `once` forms are `manual`), and override migration. `Evaluator` (7.1.3)
owns `Namespace`, `DepGraph`, `Vm`, staging and the `FormGen` counter; `eval_form` runs one expanded form. It also owns
`SourceLoader` and the `load` native (7.1.3 "Source loading"), with `NoopHost` as the default loader.

## Non-Goals

- No checker call inside `eval_form` or `load` yet: ME-INTEGRATE inserts `check` (7.1.1) into both. Until then the
  pipeline here is expand -> compile -> run (reader and expander are called by `load` on the loaded text).
- No slot table, scheduler, real host loaders or file I/O (TASK-007/008). No `sample` native (ME-INTEGRATE).
- No pattern values: tests bind plain values or test-native results to `d1`..`d9` through the ME-VM staging natives.

## writePaths (exclusive)

- `src/ns/depgraph.rs`, `src/ns/journal.rs`, `src/ns/evaluator.rs`, `src/ns/load.rs`
- `src/ns/tests/reactive_basic.rs`, `src/ns/tests/reactive_traces.rs`, `src/ns/tests/reactive_recovery.rs`,
  `src/ns/tests/rebuild.rs`, `src/ns/tests/evaluator.rs`, `src/ns/tests/load.rs`
- `impl-plans/active/vactr-middle-reactive.md`

## sharedPaths

- `src/ns/mod.rs` and `src/ns/tests/mod.rs` (created by ME-VM): declare the new modules. Pre/post hash both.

## File-Level Changes (signatures only)

- `depgraph.rs`: `DepGraph` with per-form records: committed edge set, attempt edge set, recovery subscriptions, state
  `Clean | Recomputed | Failed | Blocked { on: SlotKey }`, write set, `non_replayable` flag; topological rounds with
  definition-order tie-break; cycle detection -> `dependency-cycle` diagnostic with previous values retained.
- `journal.rs`: `PassJournal` recording pre-pass (value, version, owner) per written slot; provisional commits;
  end-of-pass failure-sensitive validation; `restore(form)` and removal of that form's staged intents from every round.
- `evaluator.rs`: `Evaluator { ns, graph, vm, staging, form_gen, loader: Box<dyn SourceLoader>, sink: Box<dyn EffectSink> }`;
  `Evaluator::new(prelude, loader, sink)`; `eval_form(&mut self, form: &Node) -> FormOutcome { value: Result<Value, Failure>, diags: Vec<Diagnostic>, form_gen: FormGen }`;
  a top-level `upd` or redefinition triggers the 5.6 pass; `queue_upd(name, Value)` + `run_pass()` coalesce
  controller-rate updates latest-wins per tick; the post-pass validated `bindings` batch is ONE `StagedEffect::Bindings`;
  `form_state(name) -> FormState` and `edges(name)` accessors for tests; `reeval`-tier tweak writes re-evaluate the owning
  form through the same pass.
- `load.rs`: `trait SourceLoader { fn read(&mut self, path: &PathVal) -> Result<(FileId, Rc<str>), Failure>; }`;
  `NoopHost` (every `read` is `Failure(host-unavailable)`); the `load` native registered against its table entry:
  reads the text, runs read -> expand -> compile -> run over its forms in a FRESH scope whose parent is the prelude (a new
  session-level `Namespace` sharing the prelude, never the caller's session), returns the last top-level form's value
  (`nil` for an empty file); a reader or expander error or a run failure -> `Failure(load-failed)` carrying the first
  cause; each nested `load` counts as one Rust-level re-entry (7.1.5), so a cycle ends in `depth-exceeded`; `load` in
  Query mode fails `effect-in-query`. The loaded namespace's bindings are not visible to the caller.

## Required Tests (each named after its vactr-core.md TASK-005 case)

- `reactive_basic.rs`: `var root 60`; `let raised + root 7`; a form binding `d1` from `raised` — `upd root 62`
  recomputes `raised` to 69, rebuilds the binding form, stages the `d1` re-bind, and marks the display batch (asserted on
  `RecordingSink`); a transitive chain and a diamond propagate once per node in topological order; equality cutoff (an
  `upd` producing an unchanged value schedules no dependents; recompute counters asserted; unrelated branches untouched);
  a var read only through a Late capture gives no rebuild; a failed recomputation keeps the previous value and binding
  and reports origin; controller-rate `queue_upd` streams coalesce latest-wins.
- `reactive_traces.rs`: CHANGING-EDGE (A/B/switch/root: dirty-read abort, final A=20 B=20, final edge sets, only final
  intents staged); genuine cycle -> `dependency-cycle`, previous values kept; FAILED-DIAMOND (left fails at root=0, total
  Blocked on left, no intent staged; repairing root recovers); ABORT/RETRY-FAILURE trace 1; REACHABLE
  PROVISIONAL-ROLLBACK trace 2 (X/Z/Y over `var n`); LATE-FAILURE trace 3.
- `reactive_recovery.rs`: CONDITIONAL-UNBLOCKING; SWITCH-TOWARD (recovery subscription); STATUS-RECOVERY bypasses the
  equality cutoff; NEWLY-DISCOVERED-SELECTOR (attempt edge set {a, b}); ORDINARY-FAILURE RECOVERY (attempt edges wake a
  Failed form with empty subscriptions).
- `rebuild.rs`: a partially superseded multi-output form (binds `d1` and `d2`, another form replaces `d1`) is ineligible
  and cannot steal `d1` back; its sites report `manual`; a form that ran `once` is excluded and reports `manual`; a
  bind-then-fail form releases nothing and keeps the old binding; a diamond rebuilds each shared descendant once; a true
  back-edge ends with the cycle diagnostic; repeated controller updates on a computed literal converge via override
  inheritance; a `reeval`-tier tweak write re-evaluates its owning form.
- `evaluator.rs`: `eval_form` returns the value and `FormGen` increments; a failing form leaves the session usable.
- `load.rs` (in-memory `MapLoader` in the test): last-expression value; the loaded file cannot see the caller's session
  names (`undefined-name` failure) but sees the prelude; the caller cannot see the loaded file's bindings; a relative
  path inside the loaded file carries the loaded file's `FileId`; empty file -> `nil`; reader error -> `load-failed`;
  `a.vact` loading `b.vact` loading `a.vact` -> `depth-exceeded` (no stack overflow); `NoopHost` -> `host-unavailable`;
  `load` inside a query closure -> `effect-in-query`.

## Invariants

- Nothing host-visible leaves a pass before validation; a failed or restored form's intents never reach the sink.
- The five traces above are asserted exactly as vactr-core.md TASK-005 words them; if the implementation disagrees
  with the design trace, stop and report it (design wins; do not weaken the assertion).
- Files under 800 lines (`evaluator.rs`, `depgraph.rs`, `journal.rs` are separate for this reason).
- No `std::{thread,fs,time,net,process}` (the loader is a trait); no dependency.

## Edit Protocol

Identical to ME-MASKS "Edit Protocol (common to every ME plan)", evidence under
`tmp/me-middle-20260925-s175/ME-REACTIVE/attempt-<n>/`. May run while ME-CHECK and ME-PATTERN are still running;
never edit their files.

## Verification

The ME-MASKS table and log rule with `<plan>` = `reactive`: V1, V2, V3, V3t, V3f, V6a, V6b, V4 (under 800), V5, V7, V8.

## Completion Criteria (map to vactr-core.md TASK-005)

- [x] `DepGraph`, `PassJournal`, `Evaluator` implement design 5.6 revised and 7.1.3
- [x] Every reactive-propagation case and trace listed above passes (TASK-005 reactive criterion)
- [x] Rebuild tests pass (TASK-005 rebuild criterion); `reeval`-tier tweak writes re-evaluate the owning form
- [x] `SourceLoader`, `NoopHost` and `load` behave as listed (fresh prelude child scope, last expression, failures)
- [x] V1-V8 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one entry per session: work done, design differences, hash/intent paths, evidence per row, blockers.)

### Session: 2026-09-25 s178 (attempt-1, step6-implement)

**Work done**
- `src/ns/depgraph.rs` (450 lines): `FormId`, `FormState {Clean, Recomputed, Failed(Failure), Blocked{on}}`,
  `WriteItem`, `Edge`, `FormRec` (committed edges with read versions, attempt edge set, recovery subscriptions,
  write set, `non_replayable`, run counter), `DepGraph` (active-owner registry for names and playing slots,
  `claim`, `is_current`/`eligible` over the COMPLETE write set, `dependents`, `status_dependents`), and `Schedule`
  (rounds, definition-order queue, dirty-read ordering constraints with constraint-cycle detection, round bound =
  distinct scheduled forms + 1).
- `src/ns/journal.rs` (646 lines): `SlotSnap`/`Snapshot` (whole-form transaction rollback of value, version, kind,
  owner), `PassJournal` (first-write pre-pass journal, `(slot, version) -> writer`, pass-level staged effects that a
  re-run supersedes, `discard` = restore + drop from every round) and the pass driver `Evaluator::propagate`
  (`rebuild`, deref-gate outcomes, status events in both directions, round-end stale reads, cycle stop,
  failure-sensitive validation to fixpoint, release with ONE `bindings` batch last).
- `src/ns/evaluator.rs` (723 lines): `Evaluator::new(prelude, loader, sink)`, `eval_form`, `eval_str`, `queue_upd` +
  `run_pass` (latest-wins coalescing), `set_tweak` (Reeval owner rebuild, binding-site dependents, Manual no pass),
  `site_tier`, the read-observer gate, override migration, accessors (`form_state`, `edges`, `attempt_edges`,
  `subscriptions`, `runs`, `sites_of`, `form_of`, `form_of_bind`, `last_pass`, `vm_and_ns`).
- `src/ns/load.rs` (155 lines): `SourceLoader`, `NoopHost`, `LoaderHost` (VM host capability), `read_forms`, the
  `load` native (registered by `Evaluator::new`), fresh `Namespace::with_prelude` scope, last value / `nil`,
  `load-failed` with the first cause, `depth-exceeded` passed through, `effect-in-query`, observer suspended.
- Tests (44 new): `reactive_basic.rs` 7 (+ shared `Harness`, `SharedSink`, `MapLoader`), `reactive_traces.rs` 6,
  `reactive_recovery.rs` 6, `rebuild.rs` 9, `evaluator.rs` 5, `load.rs` 11.
- Shared edits (intent in attempt-1/intent.md, pre/post hashes recorded): `src/ns/mod.rs` += `pub mod depgraph;
  evaluator; journal; load;`; `src/ns/tests/mod.rs` += `mod evaluator; load; reactive_basic; reactive_recovery;
  reactive_traces; rebuild;`. All ME-VM lines kept (pre-edit hashes matched ME-VM final-hashes).

**Design differences and decisions (design wins; none weakens a trace assertion)**
- Graph nodes are forms with a non-empty write set (a name defined or a slot bound). Plain expressions and
  top-level `upd` forms are commands: never recorded, never replayed. A form that `upd`s a global it does not
  define, or stages `once`/`at`/`print`/`hush`/`stop`, is non-replayable (sites `manual`); inside a rebuild the same
  is `Failure(effect-in-rebuild)` (the second barrier; checked on the staged buffer, since `EffectMode` has no rebuild
  mode and `vm/` is not this plan's). `Tempo` effects are replayable.
- A rebuild re-evaluates the stored expanded form under a FRESH `FormGen` (fresh tweak slots, section 13), with
  override migration by index/type/origin; the old generation's sites retire when the rebuild validates.
- `FormState::Blocked { on: SymId }` (the plan text says `SlotKey`; `stage::SlotKey` names playing slots, so the
  blocking origin is the read slot's name). A genuine cycle is `Failed` with `FailCode::Blocked` ("dependency cycle
  through ...") plus the `dependency-cycle` diagnostic: 7.1.6 has no cycle `FailCode` (closest listed code, finding).
- The six design traces are asserted with forms that both define and bind (`let x d1 {..}`), matching the design's
  "X ... staging its slot intent"; the FAILED-DIAMOND case keeps a separate `d1 total` binder.
- A standalone `eval_form` read of a `Failed`/`Blocked` owner's slot fails `blocked` (no mixing of a failed
  branch's retained value); re-evaluating a failed form's text supersedes it and emits the recovery status event.
- Dirty-read constraints take precedence over committed-edge order in `Schedule::pick` (committed edges are the
  heuristic, the deref gate the correctness check), which the CHANGING-EDGE trace requires.

**Evidence (evidence root tmp/me-middle-20260925-s175/ME-REACTIVE/attempt-1/)**
- V1 target/fe-logs/reactive-build-s178-1.log exit=0; V2 reactive-clippy-s178-1.log exit=0.
- V3 reactive-nextest-s178-1.log exit=0, 453 run / 453 passed; V3t reactive-cargotest-s178-1.log exit=0,
  445 + 8 passed; V3f reactive-fixtures-s178-1.log exit=0, 8/8.
- V6a reactive-wasm32-s178-1.log exit=0; V6b reactive-wasm32-hostwasm-s178-1.log exit=0.
- V4 v4-linecount.txt: largest src/compile/compiler.rs 792; owned max evaluator.rs 723 (owned-linecount.txt).
- V5 v5-stdio.txt `none`; V7 v7-deps.txt empty; V8 v8-rustfmt.txt exit=0 (10 owned leaf files; the shared
  mod.rs files are not rustfmt-run because rustfmt recurses into other plans' child modules; ME-FINAL runs crate fmt).
- final-hashes.txt written.

**Repair requests / residual risks (routed to ME-INTEGRATE)**
- `SlotSnap::restore` cannot UNBIND a slot that was a reserved forward reference before a failed attempt defined
  it (no `Namespace` unbind API; `namespace.rs` is ME-VM's). It restores `nil` with the old version. Not reachable
  by the tests (the compiler validates destructuring before any `DefGlobal`); ME-INTEGRATE may add an unbind.
- ME-VM request "failed form does not undo DefGlobal writes": answered by the whole-form `Snapshot` rollback in
  `Evaluator::attempt` (test `a_failed_standalone_form_rolls_back_its_namespace_writes` uses a partial `upd`).
- ME-VM request "compiled forms keep the prelude slot after a later shadow": by the 5.6 scope model a later session
  binding does not retarget earlier forms; prelude reads record no edge, and `sound-kit` is read dynamically by
  `QueryVm::sound_kit` (ME-INTEGRATE). A rebuild recompiles and so resolves afresh. No change here.
- ME-INTEGRATE inserts `check` into `eval_form`/`load` (7.1.1) and implements `QueryVm` for `Vm` using
  `Evaluator::vm_and_ns`; `load` is registered by `Evaluator::new` (not by `Prelude::core`).

### Session: 2026-09-25 s179 (attempt-2, step6-implement re-dispatch)

**Work done**
- Re-dispatch after the s178 join held the attempt-1 payload (implementation-progress-check: its
  priorVerification listed a superseded development run, `cargo test --lib ns::tests::reactive_traces`, exit 101,
  from before the trace fixes; the same filter reran exit 0, 6/6, in target/fe-logs/reconcile-reactive-traces-s178.log).
- No source change: all 13 owned/shared files matched attempt-1/final-hashes.txt before this attempt
  (attempt-2/pre-edit-hashes.txt). The only edit is this progress-log entry (attempt-2/intent.md).
- Bounded self-check: the six trace tests in `reactive_traces.rs` and the recovery cases in `reactive_recovery.rs`
  assert the vactr-core.md TASK-005 wording (final values, states, edge sets, attempt edges, subscriptions,
  no staged intent of failed/restored forms); no new finding.

**Evidence (fresh full-tree run, evidence root tmp/me-middle-20260925-s175/ME-REACTIVE/attempt-2/)**
- V1 target/fe-logs/reactive-build-s179-1.log exit=0; V2 reactive-clippy-s179-1.log exit=0.
- V3 reactive-nextest-s179-1.log exit=0, 453 run / 453 passed; V3t reactive-cargotest-s179-1.log exit=0,
  445 + 8 passed, 0 failed; V3f reactive-fixtures-s179-1.log exit=0, 8/8.
- V6a reactive-wasm32-s179-1.log exit=0; V6b reactive-wasm32-hostwasm-s179-1.log exit=0.
- V4 v4-linecount.txt: largest src/compile/compiler.rs 792; owned max evaluator.rs 723 (owned-linecount.txt).
- V5 v5-stdio.txt `none`; V7 v7-deps.txt empty; V8 v8-rustfmt.txt exit=0 (10 owned leaf files).
- final-hashes.txt written. Formal test-integrity, adversarial and integration review remain downstream.

## Related Plans

- **Parent**: impl-plans/active/vactr-core.md (TASK-005)
- **Previous**: vactr-middle-vm.md
- **Next**: vactr-middle-integrate.md

### OUTPUT CONTRACT NOTE (operator, 2026-09-25, after the ME-PATTERN attempt-1 failure)

- `planId` belongs ONLY in the step6-implement output payload. The
  step6-test-integrity-check and step7-adversarial-review outputs MUST NOT
  contain `planId` (their contracts reject additional properties; ME-PATTERN
  attempt 1 failed with "output contract $.planId additional property is not
  allowed" after a green gate).
- The adversarial-review output MUST contain the `findings` array (empty when
  none) and the integration-review output MUST contain `needs_revision`.
