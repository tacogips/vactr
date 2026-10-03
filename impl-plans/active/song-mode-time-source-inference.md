# Pattern time-source inference implementation plan

**Plan ID**: SONG-TIME-SOURCE-INFERENCE
**Status**: In Progress
**Design Reference**: [Part editing](../../design-docs/specs/design-song-mode.md#editing-contract), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose and dependencies

Actual checked function-returned source composition currently fails before route
preparation. Pattern coercion handles zero-argument thunks, but treats a singleton
time function as the element itself. Slice consequently infers Pattern<Fn<time,
source>> and valid song transforms fail the existing sound/control contract.

- **Consumer**: [Fixed callable sources](song-mode-fixed-callable-sources.md).
- **Depends on**: Existing trailed unifier and checked candidate pipeline.
- **Subsequent**: Full routing, host owner, finite scheduler and playback.

Keep Slice's preserving PAT_PP signature and transform admission intact. Repair
the documented function-of-time pattern coercion; do not widen Any or bypass
purity, finite result, arity, query or route authority checks.

## Exact Rust manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/types/unify.rs` | Coherent singleton time-function step coercion | In Progress |
| `src/types/song_rules.rs` | Preserve zero-argument lazy sound compatibility alongside coherent time-function checks | In Progress |
| `src/types/tests/ty.rs` | Actual type-variable relationship and rollback tests | In Progress |
| `tests/song_checker.rs` | Genuine checker/evaluator candidate regressions | In Progress |

Four paths only, every touched Rust below1000. song_checker currently967 lines;
keep new cases concise or request a root manifest amendment before splitting.
Existing four public callable fixtures remain unchanged for independent proof.

### Compatibility repair release: 2026-10-03

The broad legacy fixture observes an extra type-mismatch for the documented
zero-argument `kick-sound` function. `sound_source` bypasses normal pattern
coercion and requires one argument, contradicting the preserved zero-argument
contract. Amend this manifest with song_rules.rs before editing. Admit a
zero-argument function only through the existing bounded sound result view and
one coherent function unification trial. Preserve the current Ratio-compatible
one-argument relation, rollback, purity, and rejection of Any/numeric/finite
results and other arities. Extend existing checker loops with genuine positive
sound/rest and negative numeric/Any/Part thunks without crossing1000 lines.
The old spec diagnostic expectation stays intact. Source changes are released
only after the existing whole-library process83101 reaches terminal and its
held input hashes are captured; no completion claim precedes verification.

```rust
impl Unifier {
    fn unify_step(&mut self, elem: &Ty, actual: &Ty, depth: u32) -> Result<(), Clash>;
}
```

No new public API or dependencies. The current signature is retained.

## Type and ownership contracts

- A singleton time source is checked as one coherent function relationship:
  Ratio input and Pattern<element> result. Retain the relationship between shared
  time/result variables; independently accepting each side would be unsound.
- Reject explicit Any or non-time input. Preserve existing numeric time semantics;
  the actual query invokes callbacks with exact Value::Ratio (pattern/eval.rs432).
- Returned patterns, lists and optional/rest values retain recursive existing
  element coercion. Finite Part/Song/EventHandle results remain invalid patterns.
- Preserve zero-argument thunk compatibility and existing other-arity behavior.
- Failed try_unify restores every variable binding; no partial narrowing survives.
- Actual sound/control transform admission and effect rejection remain unchanged.
  Any result must not manufacture sound authority through this new function case.

## Tasks

### TASK-001: Coherent time-source coercion
**Status**: In Progress
**Parallelizable**: Yes (independent of host preparation source paths)

- [ ] Singleton time functions preserve the expected element relationship.
- [ ] Explicit Any/non-time input, wrong arity and finite results stay rejected.
- [ ] Zero-argument compatibility, bounded depth and trailed rollback preserved.

### TASK-002: Genuine inference and public checker witnesses
**Status**: In Progress
**Parallelizable**: No (depends on TASK-001)

- [ ] Actual indexed Slice getter checks as a sound/control-preserving callback.
- [ ] Inline and named getters, nested patterns/rests and independent captures.
- [ ] Wrong time type/arity, shared numeric time-result variable, Any escape,
      numeric transform output, finite result and impure callback remain invalid.
- [ ] Unit variables resolve to actual time/element types and failed trials roll back.

### TASK-003: Independent joined acceptance
**Status**: In Progress
**Parallelizable**: No (depends on TASK-002)

- [ ] Held exact sources, scoped formatting and line limits.
- [ ] Native/browser checks, strict Clippy and actual nonempty unifier/checker tests.
- [ ] Four unchanged public callable fixtures pass actual candidate, route,
      immutable original, closed IR and exact shared quota/depth assertions.
- [ ] Existing broader source/timing/routing/snapshot/host regressions pass.

## Progress log

### 2026-10-03: Zero-argument sound compatibility source hold

The conditional source gate is satisfied: original whole-library process83101
terminated0 with2016passes,0failures and2ignored; the checker sealed unchanged
old918inputs in `/tmp/vactr-song-final-posthashes-001.json` (SHA
9379b93ca86ed7967fdfc43cd5548f89bbb0362c359eee1de155d76288f5ff5a).
Before editing, song_rules.rs SHA5456a664a0f1e9efcb95256d6844703d5d7a0c9a7f9a5690c7bf6c1697e41fd9
and song_checker.rs SHA0f0fb70c50b31e18e8b8097860cf17d80e3f89bfa83c111d13144c6e264f1c99
were recorded. Only these two Rust files changed.

`sound_source` now permits a zero-argument function through the unchanged bounded
sound result view and one coherent `Fn([], result)` unification trial. The
one-time-argument branch retains its Ratio-compatible guard and complete shared
function trial; other arities, numeric/Any/finite results remain refused.
No unifier, effect guard, or specification diagnostic oracle changed.
The existing `lazy_sound_time_callback_checks_one_coherent_function` evaluator
loops now include genuine zero-argument keyword/rest successes and numeric,
declared-Any and Part-result failures. Existing shared-variable and time-function
cases remain intact. Scoped formatting succeeded; no author Cargo ran.
These new cases require independent execution before acceptance. Overall
inference/full-song completion is not inferred from this source checkpoint.

### 2026-10-02: Source-grounded public pipeline diagnosis

ROOT0521 joined verification passed55session and17snapshot tests, then all4public
callable fixtures failed sound/control callback typing before route checks.
Specialized source audit found unify.rs497 only unwraps zero-argument functions,
despite the documented function-of-time rule. Root inspected the unifier, correct
transform contract and exact Ratio runtime dispatch. This Ready plan authorizes
no behavioral acceptance; complete song mode remains unfinished.

### Implementation refinement before source

Retain `unify_step` signature. The singleton rule first checks actual shallow
time input is a variable or existing numeric time type, never Any/non-time.
A bounded wrapper walk rejects explicit Any returned through Pattern/List/Opt;
unbound variables remain related to the same expected element. The full
Ratio→Pattern<element> function trial then preserves time/result relationships.
No effect or song-transform validation is changed. Unit tests exercise exact
bindings, rest/nested wrappers, malformed time/result types and failed rollback;
public checker scripts exercise real named/inline Slice callbacks and invalid
numeric/finite/impure/Any escape scenarios. All original public getter tests
remain unchanged.

### Source-ready checkpoint — ROOT0524

Actual `unify_step` now checks the singleton input/result together using the
existing trailed function unifier. Explicit Any/non-time inputs and Any results
through Pattern/List/Opt are refused; numeric time compatibility, finite-result
rejection and zero-argument thunks remain. No PAT_PP, transform-contract, purity
or callback execution code changed. The design link now uses #editing-contract.

Written unit fixtures: `time_pattern_source_preserves_time_and_element_relationship`,
`time_pattern_sources_keep_wrapped_results_thunks_and_numeric_time`,
`invalid_time_pattern_sources_and_shared_numeric_result_roll_back`.
Written genuine checker fixtures: `selected_slice_time_getters_preserve_sound_control_types`,
`selected_slice_time_getters_cannot_escape_transform_contract`. These include
actual named/inline Slice code, independent captured Parts, nested Pattern/rest
results and invalid time types/arity/finite/Any/numeric/shared-variable/purity
cases. Assertions remain obligations pending independent execution.

All three touched Rust files remain below1000; scoped rustfmt/check succeeded.
No author Cargo or nextest was executed. The four public callable-source fixtures
are unchanged and must execute through the real checked candidate pipeline in
the joined checker. Full song/owner/geometry/scheduler/playback remains unfinished.

### Actual ROOT0527 failure and scoped effect dependency

Original64728 terminated101. Root audited native/browser/Clippy successes,9type
unit passes and30checker passes; the one new effectful Slice time-operand
negative was unexpectedly accepted. No assertion is removed or weakened.
Actual inherited Late/Fn-only guard skipped the Value subject's newly admitted
function-of-time. [Query-time source effects](song-mode-time-source-effects.md)
now owns its exact four-path source repair. It leaves this parent's unifier and
type tests held unchanged and preserves existing named-transform EffectInQuery
semantics. New source is authored, not behaviorally accepted; joined checker
waits both this child and the separate consuming host owner holds.

All four public fixed-callable source fixtures remain unchanged. Their real
checked candidate/routing/closure/quota evidence is still required; no full
inference/callable/host/playback completion is claimed.
