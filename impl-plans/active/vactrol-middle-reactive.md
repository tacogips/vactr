# Vactrol Middle End: Reactive Dependency Graph, Evaluator and load (ME-REACTIVE) Implementation Plan

**planId**: ME-REACTIVE (implements the reactive part of vactrol-core.md TASK-005, plus `load` and the `NoopHost` stub)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md sections 5.6 (revised reactive graph, pass journal, rounds, Failed/Blocked, attempt edge sets, status events, equality cutoff, rebuild eligibility), 7.1.3 (Staged effects, Top-level driver, Source loading), 7.1.5, 13 (reeval tier); lang-reference.md section 4 (`load`)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/2
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
- `impl-plans/active/vactrol-middle-reactive.md`

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

## Required Tests (each named after its vactrol-core.md TASK-005 case)

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
- The five traces above are asserted exactly as vactrol-core.md TASK-005 words them; if the implementation disagrees
  with the design trace, stop and report it (design wins; do not weaken the assertion).
- Files under 800 lines (`evaluator.rs`, `depgraph.rs`, `journal.rs` are separate for this reason).
- No `std::{thread,fs,time,net,process}` (the loader is a trait); no dependency.

## Edit Protocol

Identical to ME-MASKS "Edit Protocol (common to every ME plan)", evidence under
`tmp/me-middle-20260925-s175/ME-REACTIVE/attempt-<n>/`. May run while ME-CHECK and ME-PATTERN are still running;
never edit their files.

## Verification

The ME-MASKS table and log rule with `<plan>` = `reactive`: V1, V2, V3, V3t, V3f, V6a, V6b, V4 (under 800), V5, V7, V8.

## Completion Criteria (map to vactrol-core.md TASK-005)

- [ ] `DepGraph`, `PassJournal`, `Evaluator` implement design 5.6 revised and 7.1.3
- [ ] Every reactive-propagation case and trace listed above passes (TASK-005 reactive criterion)
- [ ] Rebuild tests pass (TASK-005 rebuild criterion); `reeval`-tier tweak writes re-evaluate the owning form
- [ ] `SourceLoader`, `NoopHost` and `load` behave as listed (fresh prelude child scope, last expression, failures)
- [ ] V1-V8 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one entry per session: work done, design differences, hash/intent paths, evidence per row, blockers.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-005)
- **Previous**: vactrol-middle-vm.md
- **Next**: vactrol-middle-integrate.md
