# Query-time source effect diagnostics implementation plan

**Plan ID**: SONG-TIME-SOURCE-EFFECTS
**Status**: In Progress
**Design Reference**: [Editing contract](../../design-docs/specs/design-song-mode.md#editing-contract), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose and related plans

The coherent time-source inference exposes a missing purity diagnostic. Slice's
Value subject can be a function executed at query time, but the pattern effect
guard currently scans only Late/Fn masks. The genuine negative callback fixture
accepts an effectful query function. Preserve that fixture and repair the guard.

- **Depends on**: [Time-source inference](song-mode-time-source-inference.md).
- **Consumer**: [Fixed callable sources](song-mode-fixed-callable-sources.md).
- **Independent source wave**: Consuming host preparation.

This is a bounded diagnostic extension. Existing VM query effect restrictions,
sound/control transform validation, finite result rejection and Any narrowing
remain required. Absence of a known local effect is not imported purity evidence.

## Exact Rust manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/types/infer_call.rs` | Query-time Value operand guard and resolved local effect scan | In Progress |
| `src/types/infer.rs` | Actual function/let/alias latent effect propagation | In Progress |
| `src/types/scope.rs` | Scoped binding effect fact and update plumbing | In Progress |
| `tests/song_checker.rs` | Genuine positive/negative callback regressions | In Progress |

Four paths only. Every touched Rust stays below1000; existing checker894 and
infer_call831 have limited headroom. Extra paths require a root amendment before
writes. Parent inference's unifier and type tests remain held unchanged.

```rust
pub(crate) struct Binding {
    // Existing binding fields remain.
    pub(crate) query_effect: bool,
}
impl Scopes {
    pub(crate) fn update(
        &mut self, name: &str, scheme: Scheme, extra: BindExtra, query_effect: bool,
    );
}
```

Use the actual existing scope container name in source; this declaration fixes
the update contract and owned fact rather than adding a second container.

## Required behavior

- For a pattern-returning native, additionally inspect Value operands that are
  actual singleton functions in a declared Pattern parameter position. Use the
  existing effect-in-pattern diagnostic. Eager scalar or Pattern-valued Value
  operands retain their previous behavior.
- Compute a function's known latent effects while its parameter scope is open;
  exact shadowed symbols must not resolve to unrelated effectful prelude natives.
- Carry known effects through actual local named functions, lambdas and aliases
  only when the inferred value is a function. Preserve keyword/mask metadata and
  existing type relationships. Do not mark unrelated functions by name guessing.
- Resolve exact local bindings and unshadowed prelude callable symbols in
  contains_query_effect alongside direct native calls. Preserve contains_effect
  for established diagnostics. Keep scans bounded and never execute callbacks.
- Keep the original negative d1 fixture and the four public callable witnesses.
  Cross-document unknown functions remain subject to existing runtime query
  restrictions; this scoped fact does not certify their purity.

## Tasks

### TASK-001: Query-time Value guard and scoped metadata
**Status**: In Progress
**Parallelizable**: Yes (independent of owner eight source paths)

- [ ] Value singleton functions in Pattern positions receive effect diagnostics.
- [ ] Exact local named/aliased effects propagated with correct shadowing.
- [ ] Existing metadata, ordinary eager values, purity and type rules preserved.

### TASK-002: Genuine diagnostics and compatibility fixtures
**Status**: In Progress
**Parallelizable**: No (depends on TASK-001)

- [ ] Original effectful Slice time-source negative fixture rejects unchanged.
- [ ] Named/inline/aliased effectful getters reject; pure aliases remain valid.
- [ ] Ordinary eager Value construction and native-name shadowing remain valid.
- [ ] Existing numeric/Any/arity/finite/purity and callable candidate fixtures remain.

### TASK-003: Independent joined acceptance
**Status**: In Progress
**Parallelizable**: No (depends on TASK-002 and owner hold)

- [ ] Exact held sources, scoped format/line limits and mandatory checker.
- [ ] Native/browser/strictClippy and complete actual inference/checker inventories.
- [ ] Genuine public callable, routing/snapshot/host/owner witnesses exercised.
- [ ] No full-goal completion claim from scoped diagnostics.

## Progress log

### 2026-10-02: Actual negative fixture exposed the missing guard

ROOT0527 original64728 exited101 after native/browser/Clippy passed,9unifier tests
and30checker tests passed. The unchanged effectful Slice getter unexpectedly
checked. Root verified all914inputs and raw logs. Source audit found the Value
mask exclusion and missing scoped named/alias effect fact. No negative test was
removed; implementation and behavioral verification remain required.

### Necessary same-path lexical helper refinement

`native_rules` receives its already selected instantiated parameter slice
`params: &[Ty]`, so Value operands are checked against actual Pattern positions
rather than an unrelated overload. `contains_effect` becomes pub(super) for
owned infer.rs use. Scopes owns a small document NodeId known-effect fact table:
`record_query_effect(&mut self, node: NodeId, effect: bool)` and
`node_query_effect(&self, node: NodeId) -> Option<bool>`. Lambda analysis is
recorded while its actual parameter scope remains open; rescanning its body after
that scope closes would wrongly resolve a shadowed native symbol. This is local
known-effect tracking, not a purity certificate for imported/unknown functions.
Binding.query_effect and alias metadata retain only actual resolved facts.

Fixture helper declaration: `query_effect_error(text: &str)` runs the real
checker and requires an actual EffectInPattern diagnostic rather than accepting
an unrelated syntax/type error. Genuine scripts cover global named functions,
local lambda aliases, transitive exact aliases, pure keyword-default aliases,
eager Value construction and shadowed native call heads.

### Existing named-transform diagnostic compatibility

Actual song_checker.rs735 requires a named effectful transform to retain its
EffectInQuery runtime rejection. Extending the ordinary direct-AST effect scan
globally would change that established outcome. Preserve `contains_effect` for
existing transform/Late/Fn diagnostics. New private
`contains_query_effect(&self, node: &Node, depth: u32) -> bool` uses exact local
latent facts for new Value singleton Pattern operands and metadata computation.
This narrow distinction does not relax the original negative Slice fixture or
allow a known effectful query operand. It preserves unrelated rejection identity.

### ROOT0530 source-ready checkpoint

Written scoped Binding.query_effect plus document NodeId lambda facts computed
before actual parameter scopes close; actual function aliases preserve their
existing keyword and forcing metadata. The Value singleton Pattern guard uses
the selected native parameter type and exact inferred function arity.
`contains_query_effect` resolves local facts only in this new guard/metadata
analysis; original `contains_effect` and named-transform EffectInQuery behavior
remain unchanged. No VM invocation or imported purity claim was introduced.

Written checker fixtures: `value_time_pattern_subjects_reject_actual_named_and_aliased_effects`,
`value_time_pattern_subjects_keep_pure_alias_metadata_and_eager_values`,
`value_time_pattern_effects_respect_actual_parameter_shadowing`. The helper
requires actual EffectInPattern diagnostics for negative fixtures. Original d1
negative and four public callable fixtures remain unmodified. Ordinary eager
Value calls, pure aliases with keyword defaults and unrelated effectful function
declarations have positive obligations. All fixtures await independent checking.

Scoped rustfmt/check succeeded; touched Rust files below1000. No author Cargo
or nextest. Exact held source follows; full goal and runtime query restrictions
remain required.


### ROOT0533 native callable alias followup release

Read-only audit found an actual supported alias gap: infer.rs native_value types
a prelude native used as a value, and compile/names.rs native_const loads
Value::Native. Thus `let alias d1` is supported. The held query-effect scan only
recognizes native Call heads; bare callable symbols lose their latent fact when
bound to an alias. Correct this supported route before joined acceptance.

This followup authorizes only infer_call.rs and tests/song_checker.rs within
the existing four-path manifest, plus this plan's progress. Resolve the actual
unshadowed prelude symbol through existing prelude_head; preserve exact local
and global shadowing and the original direct-AST diagnostic helper. Add actual
native-alias, transitive alias and shadowed-native positives/negatives; negative
fixtures must require EffectInPattern rather than an arbitrary error. Original
negative, public callable and unifier fixtures stay held. No other source,
parent-plan changes, Cargo, nextest, dependencies or Git are authorized. Fresh
source hold and mandatory joined checker are still required.

### ROOT0533 native alias source-ready followup

`contains_query_effect` now recognizes the actual unshadowed effectful native
callable through `prelude_head(node)` before traversing call expressions. Thus
a genuine `let alias d1` or `let alias print` retains its known latent effect
through existing callable binding/alias propagation. Actual local, global and
parameter shadowing still prevents prelude resolution. The original
`contains_effect` body and its established diagnostics remain unchanged.

Written genuine checker fixtures:
`value_time_pattern_native_aliases_retain_known_effects` requires actual
EffectInPattern for named getters, transitive aliases and a lambda using an
effectful native alias;
`value_time_pattern_native_alias_facts_respect_shadowed_bindings` preserves
pure session/native-name and parameter/native-name shadowing. These fixtures
await the mandatory independent checker. No author Cargo or nextest executed;
no behavioral success is claimed. All frozen companion sources remain unchanged.

### ROOT0552 original selected-source acceptance regression

Joined011 actual native/browser/strictClippy passed and all139 type tests passed;
checker35 passed and the original selected_slice_time_getters_cannot_escape_transform_contract
failed. Original failure logs remain under
`/tmp/vactr-callable-reservations-independent-011/`. No behavior beyond these
actual completed gates is inferred.

Source-grounded cause: ordinary `emit(EffectInPattern, ...)` uses the diagnostic's
default Warning severity. The original invalid helper filters Error diagnostics,
so the known effect warning does not reject compilation. The five new negative
fixtures required only the code, and therefore did not prove rejection.

Refinement before source: native_rules retains its signature and original
Late/Fn warning behavior. For its new Value singleton-function Pattern coercion,
an actual known effect is an Error with the same EffectInPattern code, using
Diagnostic::error and preserving Checker.mute for DSP no-abort contexts. Existing
contains_effect and eager named-transform EffectInQuery remain unchanged.
`query_effect_error(text: &str)` now requires both the exact code and Error
severity. Original negative source and all pure getter assertions remain intact.

### ROOT0552 scoped error-severity source-ready checkpoint

Known effects in the new Value singleton time-function Pattern context now
produce an Error diagnostic with the existing EffectInPattern code. The actual
Checker.mute guard still suppresses diagnostics for existing muted DSP bodies.
Ordinary Late/Fn argument warnings and contains_effect remain unchanged. The
negative helper requires both code and Error severity; no original test source,
expectation, type relation or public pure callable fixture was changed.

Scoped rustfmt/check succeeded; touched Rust remains below1000. No author Cargo
or nextest executed. The remaining original rejection and all strengthened new
negative cases require independent verification; no behavioral pass is claimed.

### ROOT0554 declared return authority repair declaration

Joined012 actual native/browser/Clippy passed and139 type tests passed; checker35
passed, with the original selected time-getter contract fixture still failing
only its explicitly declared `-> any` result. The prior d1 Error rejection now
passes. Original logs remain under `/tmp/vactr-callable-reservations-independent-012/`.

Source cause: fn_form validates ret_annot against body_ty but then builds the
function scheme from body_ty, dropping the written unknown result contract.
The actual unifier already refuses explicit Any time results. Before source,
refine fn_form's final function/self type to use the validated written return
annotation when present. Inferred results remain for unannotated functions;
existing parameter relationships, recursive self unification, masks, keyword
metadata and annotation errors remain unchanged. No unifier edit is needed.

Genuine fixture declaration: `declared_time_result_any_is_retained_while_pure_silence_remains_valid`
checks the actual final call type remains Any for a written unknown result and
requires pure unannotated nil/empty/optional getters to remain accepted. Existing
original unknown selected-source rejection remains unchanged.

### ROOT0554 declared-return source-ready checkpoint

The stored function type and recursive self type now share the validated written
return annotation, or the original inferred body result when unannotated. Explicit
Any therefore remains unknown at the caller; the existing coercion/unifier can
reject it without new special cases or authority narrowing. No source outside
infer.rs and the scoped checker fixture changed.

Written direct checked-return/silent compatibility fixture as declared. Original
negative remains untouched, as do the d1 Error repair and all five strengthened
query-effect fixtures. Scoped rustfmt/check succeeded; every touched Rust below1000.
No author Cargo/nextest. Actual behavior still awaits independent whole inventories.
