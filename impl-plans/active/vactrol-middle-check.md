# Vactrol Middle End: Static Checker with Inference (ME-CHECK) Implementation Plan

**planId**: ME-CHECK (implements vactrol-core.md TASK-004, except `types/masks.rs` and the native table, which ME-MASKS owns)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md sections 5.5 (masks consumed), 5.6 "Scope model", 5.7 (qualified names), 6.5.8 (path/url types), 7, 7.1.1, 7.1.4 (normative checker rules), 7.1.5, 7.1.6; architecture.md Typing and Realtime Validation
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/2
**dependsOn**: ME-MASKS (types, masks, native table, codes), ME-FRONTEND (`Atom::Path`/`Atom::Url` to type)
**Dispatch manifest**: impl-plans/active/me-middle-20260925-s175-dispatch.json

---

## Intent and Context

The checker is HM-lite with let-polymorphism over expanded kernel forms (design 7). In Live mode it never gates
compile or run: `check()` always returns a `CheckResult`, types a node it cannot type as `Any` with a diagnostic, and
never returns early (7.1.1). The same `check()` entry is reused by the LSP later, so `types/` has no LSP or VM
dependency. This plan also implements the 2026-09-25 checker rules: the parent-scope model (rebinding, shadowing,
shadows-prelude, upd-immutable), sound-kit keyword checking with the `kit:` argument, SOUND FIRST (`sound-not-first`),
`load`/`sample` typing, path/url types, U5 `bare-variant-binding`, chord qualities, `scale`/`shape` overloads (M1),
`range` subject first (M2) and `inst` header parameters as control names (M3).

## Non-Goals

- No compile or run (ME-VM). No fixture-manifest evaluation stages (ME-INTEGRATE wires `check` into `Evaluator` and the
  fixture runner).
- No Frozen mode (7.1.1). No LSP.
- No edits to `ty.rs`, `masks.rs`, `natives*.rs`, `diag.rs` or `fail.rs`. A missing code or type is a recorded finding;
  use the closest listed code (7.1.3).
- No `input-lane-operator` (bind-time, ME-PATTERN/ME-INTEGRATE) and no `dependency-cycle` (ME-REACTIVE).

## writePaths (exclusive)

- `src/types/infer.rs`, `src/types/unify.rs`, `src/types/check.rs`, `src/types/scope.rs`, `src/types/manifest.rs`, `src/types/deps.rs`
- `src/types/tests/check_basic.rs`, `src/types/tests/scope.rs`, `src/types/tests/sound.rs`, `src/types/tests/diags.rs`,
  `src/types/tests/deps.rs`, `src/types/tests/no_abort.rs`
- `impl-plans/active/vactrol-middle-check.md`

## sharedPaths

- `src/types/mod.rs` (created by ME-MASKS): add `pub mod infer; pub mod unify; pub mod check; pub mod scope; pub mod manifest; pub mod deps;` and re-export `check`, `CheckResult`, `HostManifest`.
- `src/types/tests/mod.rs` (created by ME-MASKS): declare the new test submodules.

## File-Level Changes (signatures only)

- `check.rs`: `CheckResult { pub types: HashMap<NodeId, Ty>, pub diags: Vec<Diagnostic> }` and
  `pub fn check(program: &[Node], env: &CheckEnv, manifest: &HostManifest) -> CheckResult`. `program` is the expanded
  forms of ONE document (a buffer, a spec block, or one form from `eval_form`). All its top-level forms are one session
  scope. Names already in `env.globals` are the previous session state: binding one again at top level is Live
  redefinition (no diagnostic; the new type replaces the old). Diagnostic severity comes from `DiagCode::default_severity()`.
- `unify.rs`: unification over `Ty` with occurs check; numeric-literal kind variables that default to int/float and join
  per the 6.5.3 widening lattice; the ONE subsumption rule `[sound]` accepted where `sound` is expected (7.1.4 sample bank).
- `infer.rs`: expression inference, let-polymorphism (generalize at `let`/`fn`), `?T` flow (`?` supplies the default; a
  `?T` used as `T` in arithmetic or a call is `optional-as-value`; accessors nil-pun), `any` must be narrowed by a
  match/if pattern before use (`any-not-narrowed`), thunk parameter typing via `infer_masks`, keyword literal typing
  against the enum or sound kit expected at that position, qualified names through `env.qualified` (absent ->
  `undefined-name`), overload resolution for `scale`/`shape` on the first argument's type when known (otherwise left to
  the VM). Recursion depth over the expanded tree is counted; past 1024 the subtree is `Any` with `nesting-too-deep`.
- `scope.rs`: the 5.6 scope chain `prelude -> session -> fn/block`. A child scope is a `fn` (parameters and the body's
  top-level statements share one scope), a lambda, a `{..}`/indented block, and a `match` clause (pattern bindings and
  guard). Emits `rebinding` (second binding in the same scope), `shadows-prelude` (hint; the name resolves in the native
  table), `shadowing` (warning; the name resolves to a user binding of an enclosing scope), `upd-immutable` (`upd` of a
  `let`, `fn`, parameter, pattern binding or prelude name), `undefined-name`. `inst` header parameters are control names
  and never get a scope diagnostic (M3).
- `manifest.rs`: `HostManifest { pub sounds: KeySet, pub synths: KeySet, pub controls: KeySet, pub caps: BTreeSet<HostCap> }`
  and `HostManifest::spec_default()`: every builtin sound keyword used after `s` in lang-reference.md and design-music.md
  examples (list them with the document line in a comment), the pattern control names, and all `HostCap`s. ME-INTEGRATE
  may add keys (recorded).
- `deps.rs`: `pub fn deps(form: &Node, env: &CheckEnv) -> FormDeps { pub eager: BTreeSet<Rc<str>>, pub late: BTreeSet<Rc<str>> }`:
  ADVISORY static edges (free top-level names split eager/late using the masks); runtime recording stays authoritative (5.6).

## Checker Rules to Implement (design 7.1.4 is normative; each bullet needs a test)

- Diagnostics of design 7: `literal-division-by-zero` (`/ x 0` literal), `undefined-name` (including `while`, `loop`,
  `break`, `when`, `unless`, `each`), `unknown-keyword`, `missing-variant` (match on an enum missing a variant, no `_`),
  `annotation-mismatch`, `type-mismatch`, `duplicate-key` (w, dict literal), `import-collision` (w, a name in two opens or
  an open and the prelude), `beyond-capability` (a native whose `needs` is not in `manifest.caps`), `effect-in-pattern`
  (w, an `effectful` native call inside a pattern-constructor argument), `unbounded-source` (w, `for`/`map` over an open
  range `0..` where provable), `mixed-forcing` (w, via `masks::mixed_forcing`), `latent-forcing` (w, an `Undetermined`
  mask entry at a determined-use site), `bad-slice-points` (literal manual points not strictly ascending within [0,1]),
  `clock-source-unavailable` (`use-clock :link`, 11.1).
- U5 `bare-variant-binding`: a `match` with exactly two clauses whose first lhs is `false | nil` and whose second lhs is
  one bare name resolving to a field-less enum variant (the `if S P -> T` desugaring and the hand-written equivalent).
  The message suggests `match S` with the variant and `_`, or `if {= S none}`.
- Sounds (7.1.4): `s`/`sound` takes one positional `pattern sound` argument plus optional `kit: [keyword: sound]`; any
  other named argument is `type-mismatch`. A keyword in a `sound` position (direct, in a step list, or an argument of a
  step constructor such as `alt`/`choose`) has type `sound`; a non-keyword argument is a sound value. Static keyword
  check against `manifest.sounds` (the `default-sound-kit` keys) plus `inst` names defined in the program runs only when
  the call has no `kit:` AND `sound-kit` resolves to the prelude (not in `env.globals` and not bound in the program before
  the call); a miss is `unknown-keyword`. `midi` with a literal channel outside 1..16 is `type-mismatch`.
- SOUND FIRST: a call to `s`/`sound` with two or more positional (non-pair) arguments is `sound-not-first` (message:
  `s` starts the chain; suggest `s :bd > n [0 3]`). `s {n [0 3]}` is `type-mismatch`.
- `load : fn path -> 'a` (fresh variable per call site); `sample : fn path -> sound`; a `url` given to either is
  `type-mismatch`; path literals type `path`, url literals type `url`, neither converts to or from `string`.
- Chord qualities `:maj7 :m7 :dom7 :sus4` only; another literal quality is `unknown-keyword`.
- `grid` is the pattern function; `range` is subject first (`range sine 200 2000` checks clean; there is no pinned
  `type-mismatch` for it any more).
- `struct NAME:`, `inst` and `look` are definition heads; `inst`/`look` bodies are checked for the no-abort property only.

## Required Tests (src/types/tests/)

- `check_basic.rs`: literals, lists, dicts, fn inference and let-polymorphism, numeric widening, `?T` and `?`,
  `any` narrowing, the annotated negatives `+ 1 "a"` (`type-mismatch`) and `?T` as `T` (`optional-as-value`).
- `scope.rs`: `let a 12` then `let a 13` -> `rebinding` on line 2; `var hits 0` then `var hits 5` -> `rebinding`;
  `upd a 13` on a let -> `upd-immutable`; `upd sound-kit ...` -> `upd-immutable`; `let scale 2` -> exactly one
  `shadows-prelude` hint; `let x 1` then `fn f x: x` -> `shadowing` warning; two `let y` inside one fn body -> `rebinding`;
  `let y` in a nested block under a fn binding `y` -> `shadowing`; a single-form `check` with `a` already in
  `env.globals` gives no diagnostic (Live redefinition); `inst pluck amp: float = 0.5:` gives no scope diagnostic.
- `sound.rs`: `s :bd > d1` clean; `s :not-a-sound > d1` -> `unknown-keyword`; `n [0 3] > s :bd > d1` and
  `note [:c] > s :x > d1` -> `sound-not-first`; `s [:bd909 :sd909] kit: tr909 > d1` (with `tr909` a `[keyword: sound]`
  dict) -> clean; `let sound-kit put default-sound-kit [bd: {sample ./bd/909.wav}]` then `s :bd > n [0 3] > d1` -> clean
  except one `shadows-prelude` hint; `let kick sample ./kick.wav` then `s kick > d1` clean; a pack dict
  `[bd: [sample ./a.wav sample ./b.wav] sd: sample ./sd.wav]` types `[keyword: sound]`; `load ./p.vact` then
  `put default-sound-kit my-pack` clean; `load https://x.org/p.vact` -> `type-mismatch`; `s {midi 1} > note [:c :e] > d1`
  clean; `midi 17` -> `type-mismatch`; `s :bd foo: 1` -> `type-mismatch`.
- `diags.rs`: one positive trigger per checker-emitted code listed above (including U5 on both the desugared `if` form
  and the hand-written `match`, and the chord quality `:7`-free set), each asserting code, severity and span line.
- `deps.rs`: eager vs late split for `let raised + root 7` (eager `root`) and a pattern capturing a `var` Late.
- `no_abort.rs`: `check` on every spec block (read + expand of each fence in lang-reference.md and design-music.md, the
  forms without reader/expander errors) returns without panic and with a `types` entry for every top-level form; a
  1100-deep nested expression gives `nesting-too-deep` and no panic; the same `check` entry is called directly (no LSP
  types).
- Masks-identical test: `infer_masks` on every `fn` in the spec blocks gives the same result whether or not `check` ran
  first (7.1.1).

## Invariants

- `check` never panics and never returns early; every top-level form gets a type (possibly `Any`).
- `types/` imports nothing from `vm/`, `ns/`, `compile/`, `pattern/` or `tex/`.
- Files under 800 lines; split `infer.rs` into `infer.rs` + `infer_call.rs` if it nears 800 (record it).
- No `std::{thread,fs,time,net,process}`; no dependency added.

## Edit Protocol

Identical to ME-MASKS "Edit Protocol (common to every ME plan)", evidence under
`tmp/me-middle-20260925-s175/ME-CHECK/attempt-<n>/`. This wave runs in parallel with ME-VM and ME-PATTERN in the same
working directory: a build error in a file outside this plan's paths is waited out and recorded, never edited here.

## Verification

The ME-MASKS table and log rule with `<plan>` = `check`: V1, V2, V3, V3t, V3f, V6a, V6b, V4 (under 800), V5, V7, V8.
Additionally record, inline, the test names of `src/types/tests/*` that ran (from
`CARGO_TERM_QUIET=true cargo test types::tests -- --list` into LOG(check-list)).

## Completion Criteria (map to vactrol-core.md TASK-004)

- [ ] `check()` implements design 7 and 7.1.4 with every rule above tested
- [ ] Annotated negatives (`+ 1 "a"`, `?T` as `T`, rebinding, `upd` of a `let`) produce their codes
- [ ] Scope model diagnostics (rebinding error, shadows-prelude hint, shadowing warning, upd-immutable) tested
- [ ] Sound rules (kit, static check skip, sound values, bank subsumption, sound-not-first) and load/sample/path/url typing tested
- [ ] U5 `bare-variant-binding` diagnosed on both forms
- [ ] Checker never aborts (no_abort tests); the same `check()` entry is reused by tests with no LSP dependency
- [ ] V1-V8 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one entry per session: work done, design differences, hash/intent paths, evidence per row, blockers.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-004)
- **Previous**: vactrol-middle-masks.md, vactrol-middle-frontend.md
- **Next**: vactrol-middle-integrate.md (wires `check` into `Evaluator` and fixtures)
