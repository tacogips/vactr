# Vactr Implementation Design

Supporting document to `architecture.md`: how the Vactr processor is
built. It designs the reader, static checker, bytecode VM, scheduler,
pattern engine, DSP graph, LSP, REPL, the Tauri/web/Wasm editor, visual
feedback, and controller binding, and it specifies every internal data
structure the runtime uses.

This document defines **no language syntax**. The language is specified
by `lang-reference.md`, `design-music.md`, and `design-visual.md`; the
decision log in `architecture.md` is binding. Where this document names
a surface form it is quoting those specifications. The four open
questions at the end carry **recommendations, not decisions** (section 20).

Implementation plan: `impl-plans/active/vactr-core.md`.

---

## 1. Goals and Non-Goals

**Goals (v1, Live mode, web-first)**

- One Rust core — reader, checker, VM, scheduler, pattern engine, DSP
  graph, LSP sharing the checker — that builds for native desktop and
  `wasm32-unknown-unknown`.
- A REPL and a live session that never dies from a user error.
- A dedicated editor (TypeScript frontend) that runs in the browser on
  the Wasm core and, wrapped by Tauri, as a desktop app; inline
  diagnostics, highlighting of the pattern steps and events currently
  sounding, and the feature set TidalCycles/Strudel/algorave
  performance needs (section 15).
- Controller binding as an editor/runtime affordance — never language
  syntax (section 13): the Decided editor requirements
  (architecture.md Editor Requirements): a right-pane slider panel
  over every enumerated numeric site including `inst` parameter
  defaults, mouse drag on numbers, MIDI learn, the `cc` signal and
  MIDI note input, MIDI out, MIDI clock/transport sync in and out,
  DAW-style parameter editors matched to each builtin's meaning,
  display-only step grid and piano roll, and `#@` DIRECTIVE COMMENTS
  with labels for control-panel setup persisted in the file (13.5 —
  the previously recorded authority conflict between these Decided
  directives and the no-source-annotation constraint is now
  ADJUDICATED by the author: the constraint is scoped to language
  syntax, and directives are comments the language ignores).
- Packages (architecture.md Packages, lang-reference.md modules —
  Decided, superseding "no module system in v1"): Go-style GitHub-path
  imports, git-tag versions with minimal version selection,
  `vactr.lock`, a cache, and a browser package proxy (section 5.7).
- The Decided synthesis models (sampler, analog, FM, phase
  distortion, additive, wavetable, granular), the builtin effect
  catalog with `bus`/`master` chains, and the analyzer signals
  (design-music.md sections 4-6) — sections 12.4-12.7.
- Host tiers (architecture.md Host Tiers): browser tier first, the
  native tier (the same core compiled natively inside the Tauri app,
  CoreAudio via `cpal`, CoreMIDI) second, behind capability traits;
  a missing capability is a diagnostic, never a crash.

**Non-goals (v1)**

- Frozen mode code generation (typed IR to Wasm/native) — the design
  keeps it possible (no core feature blocks it), but it is not planned
  here.
- The Cranelift JIT (desktop-only optimization, optional by decision).
- Swift GLUE (AVAudioSession, background-audio, a later AUv3
  extension) and any UniFFI bindings: per the Decided Host Tiers,
  Swift is glue only and comes with the native-tier milestone, which
  this plan orders after the browser tier per the workflow constraint
  "no CoreAudio/Swift shell in this plan" — the `cpal` native dev
  host stays in scope, the Apple glue does not.
- Ableton Link (explicitly "planned" in the specifications: the
  `ClockSource` enum represents `:link`, its implementation is a
  follow-up, and selecting it is a "not available" diagnostic).
- Native-tier-only capability EXTENSIONS (long convolution IRs, heavy
  granular densities, multichannel output, offline rendering, plugin
  hosting): the CapabilitySet advertisement and diagnostics (12.7)
  are in scope; the extended native limits ship with the native-tier
  milestone.
- Music-visual coupling (`hits`/`ctrl`/`looks` consumed by visual
  chains) — parked in `notes.md`. The editor still shows visual
  feedback ABOUT music (playing steps, events, levels, analyzers),
  which is a tooling concern, not language coupling.
- Raw GLSL, macros beyond fixed sugar, try/catch — withdrawn or
  deferred by the specifications. (The former "module system"
  exclusion is REMOVED: packages are now Decided and in scope.)

## 2. Current State and Evidence

- `Cargo.toml` is a single empty package `vactr` (lib + bin), no
  dependencies. `src/lib.rs` / `src/main.rs` are scaffold stubs. There
  is no code to migrate; compatibility constraints come only from the
  specifications.
- `design-docs/specs/*.md` carry the Decided items this design must
  respect; the indent-syntax question is resolved piecemeal in
  `lang-reference.md` (see `user-qa/pending-indent-syntax.md`,
  superseded).
- Binary/crate name `vactr`, source extension `.vact` (`command.md`).

## 3. System Overview

```
                    evaluator thread (ALL user code)
  .vact text ──> reader ──> expander ──> checker(diagnostics) 
                   │                        │
                   └──> compiler ──> bytecode ──> VM ── namespaces (late-bound vars, tweak slots)
                                                  │
                     pattern values <── prelude ──┘
                            │
        cycle clock ─── scheduler ─── slot table ─── dry-run (no-op host)
             │               │ events (POD, timestamped, latency window)
             │               ├────────────► audio thread: DSP graph, voice pool   (lock-free SPSC)
             │               ├────────────► render thread: shader + uniforms      (values only)
             │               └────────────► MIDI / OSC sinks (I/O thread)
             │
        diagnostics bus ──► session protocol ──► REPL console / editor / LSP
```

The S-expression AST is the single shared data model (architecture.md
Pipeline). The LSP and the live session share one process or a socket,
because runtime diagnostics originate in the scheduler.

## 4. Crate and Module Layout

One core crate `vactr` (the existing package), feature-gated hosts.
No workspace split in v1; the editor frontend is TypeScript under
`editor/` and consumes the core through the raw wasm export ABI of
12.8.10 (wasm-bindgen was dropped in issue #3) or the session socket.

```
src/
  value/     value.rs ratio.rs key.rs dict.rs intern.rs   # sections 5
             num.rs access.rs eq.rs print.rs              # 6.5.3 (TASK-001)
  reader/    lexer.rs layout.rs sexpr.rs span.rs          # section 6
             node.rs line.rs import.rs                    # 6.5.4 (TASK-002)
             pathlit.rs                                   # 6.5.8 (issue #2)
  expand/    expander.rs sugar.rs kernel.rs               # section 6.4, 6.5.5
  types/     ty.rs infer.rs diag.rs manifest.rs           # section 7
             masks.rs natives.rs deps.rs check.rs         # 7.1 (TASK-004)
  ns/        namespace.rs tweak.rs                        # section 5.6
             stage.rs depgraph.rs journal.rs evaluator.rs # 7.1 (TASK-005)
             load.rs                                      # 7.1.3 (issue #2)
  compile/   compiler.rs proto.rs                         # section 8
  vm/        ops.rs frame.rs vm.rs fail.rs natives/       # section 8, 7.1
  pattern/   pat.rs step.rs query.rs combinators/ signal.rs eval.rs  # section 10, 7.1
  tex/       texnode.rs shader.rs uniforms.rs                  # section 9
  clock/     tempo.rs clock.rs                            # section 11.1, 11.7
  sched/     slots.rs runtime.rs staging.rs telemetry.rs  # section 11, 12.8.2
  dsp/       graph.rs engine.rs voice.rs ugen/ effects/   # section 12, 12.8.2
  host/      caps.rs wire.rs noop.rs native/ wasm/        # sections 11.5, 16, 12.8
  session/   session.rs eval.rs publish.rs authority.rs   # sections 13, 14, 14.5
             protocol.rs codec.rs changes.rs console.rs repl.rs
  pkg/       manifest.rs lock.rs mvs.rs digest.rs store.rs native/  # 5.7, 14.5.7
  directives/ parse.rs attach.rs labels.rs resolve.rs key.rs persist.rs  # 13.5, 14.5.8
  cli/       args.rs repl.rs run.rs serve.rs get.rs ws.rs # 14.5.10, command.md
  lsp/       server.rs analysis.rs                        # section 14.3, 14.5.11
editor/      TypeScript: CodeMirror 6 frontend, Tauri shell, worklet JS
```

| Feature flag | Pulls in | Default |
|--------------|----------|---------|
| (none) | serde, serde_json, miniz_oxide: wasm32-clean, used by the protocol codec and proxy zips (14.5.2) | always |
| `host-native` | cpal, midir (non-wasm32 targets only, 12.8.10); tungstenite + getrandom for the session socket (TASK-009, 14.5.2) | yes (desktop dev) |
| `host-wasm` | no crates: raw `extern "C"` exports (12.8.10) | no (wasm builds) |
| `lsp` | tower-lsp, tokio, and implies `host-native` (non-wasm32 only) | no |

The core modules (`value` … `dsp`) have no I/O dependencies and no
assumption of OS threads, keeping `wasm32-unknown-unknown` green from
the first commit.

Each module directory may hold more files than the table lists, provided
every file stays under the 1000-line limit. Unit tests that would push a
file over the limit go in a sibling `tests/` submodule, for example
`src/reader/tests/lexer.rs`, declared as `#[cfg(test)] mod tests;`.
Section 6.5.1 lists where the foundation types and placeholder shells
created by TASK-001 live.

## 5. Core Value Model

### 5.1 Value representation and tagging

A plain Rust enum, not NaN-boxing (tradeoff in section 19). All user
code runs on one evaluator thread, so reference counting is `Rc`, not
`Arc`; the type is deliberately `!Send`. **Invariant: a `Value` never
crosses to the audio or render thread** — handoff converts to POD
structs (section 11.4).

```rust
pub enum Value {
    Nil,
    Bool(bool),
    Int(i32), Int64(i64), Float(f32), Float64(f64),
    Ratio(Ratio64),                       // exact int64/int64
    Keyword(KwId),                        // interned
    Str(Rc<str>),
    List(Rc<ListVal>),                    // immutable; pairs are 2-lists (5.4)
    Dict(Rc<BTreeMap<Key, Value>>),       // sorted map (section 5.4)
    Struct(Rc<StructVal>),                // declared dict: type id + sorted fields
    Variant(Rc<VariantVal>),              // enum tag + fields (a tagged struct)
    Fn(Rc<Closure>),  Native(NativeId),
    Thunk(Rc<Closure>),                   // zero-arg block, forced on demand
    VarRef(VarSlotRef),                   // late-bound global var/fn/tweak (5.6)
    Pattern(Rc<Pat>),  Signal(Rc<Sig>),
    Inst(InstId),  Tex(Rc<TexNode>),      // instrument ref, visual chain (9)
    Range(RangeVal),                      // 0..8 eager list-like; 0.. lazy
}
```

Operations: construction from literals (compiler), structural equality
(`=`, deep; operands reach `=` already forced by its own call boundary
— 5.5 — and `VarRef` operands deref), ordering only where a sort key
exists, `print` formatting (integral ratios print as ints per
spec), truthiness (exactly `Nil` and `Bool(false)` are falsy),
nil-punning accessors (indexing/keying `Nil` yields `Nil`; `len nil` is 0).

### 5.2 Interning

Keywords and identifiers intern to `u32` ids in a session-global,
evaluator-thread-only table:

```rust
pub struct Interner { names: Vec<Rc<str>>, map: HashMap<Rc<str>, u32> }
pub struct KwId(u32);  pub struct SymId(u32);
```

Operations: `intern(&str) -> id`, `resolve(id) -> &str`. Keyword
comparison and dict key ordering resolve through the table
(alphabetical order is on the string, not the id).

### 5.3 Numbers and `Ratio64`

Fixed widths per the decision: `int` = i32, `int64` = i64, `float` =
f32, `float64` = f64, `ratio` = exact i64/i64. Hand-rolled (no bignum
dependency) so overflow semantics are ours:

```rust
pub struct Ratio64 { num: i64, den: i64 }   // den > 0, gcd(num,den) == 1
```

Operations: `new` (normalizes; zero denominator = failure), checked
`add/sub/mul/div` (i64 overflow = failure, like division by zero),
`floor/frac` (cycle math), ordering, conversion `to_f64` (host boundary
only). Literal adaptation (a literal takes the width its context
expects, Zig-comptime style) is a checker/compiler concern: literal AST
nodes stay kind-unconstrained until inference resolves them; dynamic
fallback is `int`/`float` as written. Widening follows the decided
lattice; narrowing is only the explicit calls `int`, `int64`, `round`.

### 5.4 Lists, pairs, and the sorted-map dict

A list is immutable; `put`/`join` build new lists. Lists are small in
live code; a persistent vector is a later optimization, not v1
(section 19).

```rust
pub struct ListVal { items: Box<[Value]>, prov: Option<Rc<ListProv>> }
pub struct ListProv { form_gen: FormGen, doc_revision: u64,
                      elems: Box<[Span]> }   // literal element spans + origin identity
```

`prov` is attached by the compiler to LIST LITERALS and travels with
the value, so a literal bound by `let kit [...]` and used later where a
pattern is expected still carries per-element provenance for step
highlighting (10.2). Derived lists (`put`, `join`, `map`, slices) carry
`prov: None`; steps without element provenance fall back to the
pattern's binding span — an honest degradation, surfaced as such in the
editor (14.4).

`key: value` is the pair `[:key value]` — a plain 2-list. A dict is
list SYNTAX over a **sorted map**: at runtime `Rc<BTreeMap<Key, Value>>`
with unique keys and key-ordered iteration, per the decision.

```rust
pub enum Key { Num(NumKey), Kw(KwId), Str(Rc<str>) }   // total order:
// numbers (numeric, across widths; NumKey normalizes) < keywords (alpha) < strings (alpha)
```

Operations on dict: `get(key) -> Value` (missing = `Nil`; callable-with-key
is this), `put` (assoc; pair elements; `&` splat = merge, last wins),
`join` (merge many), iteration as pairs in key order (`first`, `for`,
`map` see `[k v]` 2-lists), `len`. A dict literal whose pairs repeat a
key keeps the last pair (checker emits the duplicate-key diagnostic).
Every list function accepts a dict by iterating its pairs; functions
that would produce a non-pair list (`map d {[k v] -> v}`) produce a
list, not a dict. Non-key-typed values used as dict keys are a failure.

`Struct` is a declared dict: the type declaration (name, ordered
fields, defaults) lives in the namespace; the value stores fields in a
fixed, key-sorted slot array so access is an index, while iteration and
patterns behave exactly like the equivalent dict. Unknown field access
or `put` with an unknown key is a failure (closed). A `Variant` is a
struct tagged with its enum; structural equality compares tag + fields.

### 5.5 Closures and thunks

```rust
pub struct Closure { proto: Rc<FnProto>, captures: Box<[Value]> }
```

A `{}` block with no arrow compiles to `Thunk` (a zero-arg closure).

**Forcing contract (normative; implements the decided type-directed
semantics on the dynamically checked VM).** Every function value —
closure and native alike — carries a **forcing mask**: one entry per
parameter, `Value` (force a thunk argument before binding it), `Fn`
(pass the closure; the callee calls it), or `Late` (capture the
argument DEFERRED: the domain constructors — pattern, signal, `inst`,
visual — take thunks and `VarRef`s as `PParam`/`VParam` entries,
evaluated later on the evaluator thread; sections 9.1, 10.1). Natives
declare their masks in the prelude signature table; `Late` belongs to
domain natives only in v1, and propagates through user wrappers like
`Fn`.

**Mask derivation is a mandatory compilation dependency, not a
fallback.** Compiling any function ALWAYS runs the checker's local
parameter inference (`types/masks.rs`: the same unification the
diagnostics pass uses, factored to run without it): a parameter used
in call position or carrying a function-type annotation is `Fn`; and
every other DIRECTLY determined use — the parameter consumed by a
`Value`-masked native such as arithmetic or a data constructor — is
`Value`. The earlier claim that a direct-occurrence syntactic analysis
is equivalent is WITHDRAWN — direct occurrence does not see
forwarding, so it is not the contract.

**Forwarded contracts are resolved at the WRAPPER'S OWN call boundary
by chasing the live downstream bindings — never deferred into the
body, never copied at compile time.** A parameter whose determined
uses are FORWARDS — `fn later body: at 4 body` forwards `body` to
`at` — carries the mask entry `Forward{ links }`, where `links` is
the SET of (downstream callee reference, parameter position) pairs
the parameter is forwarded to (fan-out keeps one link per
destination). `Forward` is a symbolic dependency, not a decision, and
it is resolved at the moment the WRAPPER is called, BEFORE its body
runs: the VM chases each link against the CURRENT namespace (aliases
and transitive wrappers chase link by link, with a visited set; a
chase cycle or an unresolvable link degrades to `Undetermined`) and
combines the destinations' CURRENT entries into one EFFECTIVE entry
for this call:

- ANY destination `Value` -> effective `Value`: the thunk is forced
  EXACTLY ONCE at the wrapper's boundary, in left-to-right argument
  order like every other `Value` argument — so a doubly-forwarded
  argument runs its effect once (both inner calls receive the same
  forced value), a conditionally-skipped forward STILL runs it once
  (identical to the decided treatment of an ignored value argument),
  and effect order is the boundary's argument order. If another
  destination in the same set wanted `Fn`/`Late`, the checker emits a
  `mixed-forcing` diagnostic (one argument cannot be both a number
  and a function); at runtime the forced value flows and a call on a
  non-function fails normally with origin.
- ALL destinations `Fn`/`Late` -> pass unevaluated (`Fn` if any is
  `Fn`, else `Late`); the thunk reaches the inner calls intact.
- ANY destination `Undetermined` (or an unresolvable chase) ->
  effective `Undetermined` (memoized cell + `latent-forcing`
  diagnostic, below).

Because the chase happens per call against current bindings,
redefining ANY function in the chain — `at` itself, or a transitive
wrapper, between `Value`, `Fn`, and `Late` — changes the wrapper's
forcing at its very next call with no recompilation; and because the
resolution completes AT the boundary, the decided call-site semantics
("a non-function parameter receives the block's value, forced at the
call site") holds with exactly-once multiplicity for repeated,
conditional, and ignored uses alike. The rev-4 inner-call-site
deferral, which broke that multiplicity, is withdrawn.

Where inference is genuinely undetermined at a determined-use site
(the parameter flows only through `any`, or is consumed by a callee
unknown at compile time and not reducible to a `Forward` link), the
mask entry is `Undetermined`: at the demanding call boundary the
argument thunk is wrapped ONCE in a memoizing cell — later demands
run the body AT MOST ONCE, at first demand — and the compiler emits a
`latent-forcing` diagnostic so the site is visible. Memoization
exists ONLY for `Undetermined` entries; `Value`, `Fn`, `Late`, and
resolved `Forward` entries never memoize.

At `Call`, the VM resolves the callee (the late-bound lookup yields
the CURRENT function) and applies ITS mask, left to right, exactly
once per argument: `Value` forces a thunk (including arguments the
callee never reads), `Fn` and `Late` pass it unevaluated, `Forward`
is CHASED to its effective entry right there at the boundary (above)
and that effective entry is applied, `Undetermined` wraps it
memoized. Redefining any function in a forwarding chain therefore
changes forcing at the wrapper's next call, while multiplicity stays
exactly-once at the boundary. Thunks never leak into PLAIN data —
list/dict/struct constructors and arithmetic are `Value`-masked
natives — but they ARE deliberately captured by `Late` domain
parameters: `osc {* 20 {sin time}}` keeps its thunk alive per frame
exactly as `design-visual.md` requires, and `fn later body: at 4
body` passes the thunk through unevaluated because the boundary
chase of `body`'s single link resolves to `at`'s current `Fn`
parameter. A `{}` on a statement line or in a clause body compiles
to an immediate force where it stands. The checker's remaining
call-site contribution is diagnostics (including `mixed-forcing`)
and, in Frozen mode only, inlining; the runtime chase result may be
cached keyed by (destination slot, version) as a pure optimization.
Planned tests (TASK-005): an ignored effectful thunk argument runs
exactly once; a doubly-read `Value` parameter forces once; a wrapper
that forwards its parameter TWICE to a numeric consumer runs the
effectful thunk exactly once (counter test); a CONDITIONAL wrapper
whose forwarding call is skipped still forces exactly once at its
boundary; boundary effect ORDER is left-to-right across mixed
argument kinds; the forwarding wrapper passes its thunk to `at`
unevaluated; a time-varying visual argument stays deferred; an
`Undetermined` parameter runs at most once with the `latent-forcing`
diagnostic; mask derivation with the diagnostics pass disabled
produces identical masks; redefining the DIRECTLY-called callee
between `Value` and `Fn` switches at the next call; redefining only
a DOWNSTREAM callee an unchanged wrapper forwards to (change `at`
among `Value`/`Fn`/`Late`, `later` untouched) switches `later`'s
forcing at its next call; a fan-out forward whose destinations
currently disagree (`Value` vs `Fn`) forces once and carries the
`mixed-forcing` diagnostic.

### 5.6 Namespaces, late-bound vars, and tweak slots

One SESSION namespace for user code, plus one child namespace per
imported package (5.7 — the earlier "no module system" phrasing is
superseded by the Decided package system). Every top-level definition
(`let`, `var`, `fn`, `inst`, `struct`, `enum`, `look`) owns a slot;
redefinition replaces the slot's value (Clojure-style indirection,
per the Redefinition Model):

```rust
pub struct VarSlot { name: SymId, kind: SlotKind, value: RefCell<Value>, version: Cell<u64> }
pub enum SlotKind { Let, Var, Fn, Tweak }     // Tweak: section 13
pub struct Namespace { slots: Vec<Rc<VarSlot>>, by_name: HashMap<SymId, usize> }
```

Late-binding rule (implements `lang-reference.md` section 4):

- Loading a top-level `var` or `fn` pushes `Value::VarRef(slot)`. The
  VM auto-derefs a `VarRef` wherever a concrete value is demanded (same
  demand points as thunk forcing), but pattern, instrument, and signal
  constructors STORE the ref and deref at each query/event — that is
  how `upd cutoff 400` and re-running the block-form `fn kick-sound:`
  with body `:bd-tek` are heard at the next event with no re-binding.
- Loading a top-level `let` derefs immediately (a snapshot). Re-running
  the `let` line replaces the slot for future evaluations (live
  redefinition), which is neither mutation nor shadowing.
- Locals live in VM frames; `let` locals are single-assignment, `var`
  locals are frame slots `upd` may rewrite. Binding a name twice in ONE
  scope is a checker error; a child scope may shadow a parent binding
  (scope model below, 20 Q1 Decided). At the top level in Live mode,
  re-evaluating a definition is always redefinition, never an error.

`version` increments on every write; the editor and controller-binding
layer use it to refresh displayed values cheaply.

Two session-level records support live redefinition and controller
binding (section 13):

- **Form generations.** Every top-level form evaluation receives a
  monotonically increasing `FormGen(u64)`. Tweak sites, telemetry, and
  protocol messages carry it, so stale references are detectable and
  rejectable (14.4).
- **Reactive dependency graph and automatic rebuild (Decided:
  architecture.md Decision Log "Reactive dependency graph",
  reactive-namespace paragraph; lang-reference.md section 4 — the
  REQUIREMENT is Decided, the Salsa/Adapton structure is a
  recommendation; this design implements the requirement by
  GENERALIZING the machinery below, which earlier revisions defined
  for `let` only).** During each top-level evaluation the VM records
  which top-level slots — `let`, `var`, AND `fn`, any kind — were
  EAGERLY dereferenced (a `Deref` that forced the value into the
  computation: arithmetic, data construction, a call argument
  consumed as a value). Capturing a `VarRef` into a `PParam`/
  `VParam`/inst argument records NO eager edge: those are the
  authority's "dynamic edges", already live through the
  per-query/per-frame late read and the cell path (11.3) with no
  rebuild needed. The session keeps form -> eager-read slots plus
  the form's source text, and the ACTIVE-OWNER registry: for every
  slot binding and every top-level definition, the form (and
  `FormGen`) that currently owns it. TRIGGERS: any write to a
  top-level slot — redefinition of a `let`/`var`/`fn` (by re-typing
  or through a controller binding, 13) or `upd` on a top-level
  `var` — schedules the forms whose eager-read set contains that
  slot; the slot `version` counters (above) are the revision
  stamps. Dependent forms then rebuild under these rules:
  - **Eligibility over the COMPLETE replay write set, not one
    artifact.** A form's write set is every artifact its evaluation
    produces: each slot it binds AND each name it defines. A form is
    eligible to rebuild only if it is the CURRENT owner of EVERY
    member of that write set. If ANY member has been superseded by a
    later form, the form is INELIGIBLE for automatic rebuild — its
    sites report `manual` — because replaying it would overwrite an
    artifact another form now owns. This closes the multi-output
    counterexample: form A that binds both `d1` and `d2`, after form
    B replaces `d1`, no longer owns its whole write set, so changing
    A's `let` does NOT auto-replay A and cannot steal `d1` back from
    B. (Partial ownership is thus never "rebuild anyway"; it is
    "rebuild nothing automatically, surface `manual`.")
  - **Effects (replayability).** A form whose evaluation performed
    one-shot effects (`once`, `at`, top-level `print`, any direct
    host effect) is marked NON-REPLAYABLE at eval time and excluded
    from automatic rebuild; its dependent sites report the `manual`
    tier (13). As a second barrier, rebuilds run in a mode where
    one-shot effects fail, aborting that form's rebuild with a
    diagnostic — a historical `once` can never replay.
  - **Whole-form transaction covering ALL writes, not only the
    namespace.** A rebuild stages EVERY effect until whole-form
    success: namespace writes to an overlay, slot binds as STAGED
    pending-swap intents (built and dry-run but not installed),
    ownership/index updates, and the `SlotControl` revocations they
    imply. On whole-form success the transaction commits all of them
    together — overlay applied, staged binds promoted to `pending`,
    ownership advanced, revocations sent. On ANY failure the whole
    transaction is discarded: overlay dropped, staged binds retracted
    (they were never promoted to `pending`, so no half-installed
    bind survives — the bind-then-fail counterexample), ownership
    unchanged, old bindings keep playing; the failure is reported
    with origin, and downstream handling follows the
    FAILED/BLOCKED joins rule below (blocking decided at each
    dereference actually performed). SCOPE OF FINALITY: for a
    STANDALONE rebuild (manual re-eval, single-form trigger) a
    committed transaction is final and earlier committed
    transactions stand. WITHIN A REACTIVE PASS a form
    transaction's commit is PROVISIONAL until pass validation:
    its namespace writes become readable to later forms
    immediately, but ownership advancement, revocations, and
    every other host-visible effect are PASS-STAGED alongside the
    bind intents, and the namespace commit itself is JOURNALED
    and revocable — see the pass journal rule below. "Earlier
    transactions stand" therefore holds unconditionally only for
    VALIDATED commits; a provisional commit invalidated by a
    later failed retry is rolled back by the journal.
  - **DAG propagation, cycles only for true back-edges.** The
    dependency graph is a DAG in general (a shared descendant reached
    through two parents — a diamond — is normal). Propagation does a
    dependency-respecting traversal (topological order over the
    affected sub-DAG): each dependent is SCHEDULED once and rebuilt
    once, AFTER all its triggering ancestors, even when reached
    through multiple parents — a re-encounter of an already-scheduled
    (not yet rebuilt) node is DEDUPLICATED, not flagged. A cycle is
    reported only for a true back-edge: reaching a node that is an
    ANCESTOR on the current traversal path (still open). This
    distinguishes a valid diamond from an actual dependency cycle.
  - **Equality early-cutoff (adopted from the recommendation).**
    After a rebuilt form's transaction commits, each slot it wrote
    is compared structurally with its previous value (values are
    immutable, so equality is cheap); dependents are scheduled only
    from slots whose value actually CHANGED. An update that leaves
    a derived value equal stops there — unrelated and unchanged
    branches are never recomputed, satisfying "only the affected
    part". The cutoff gates VALUE propagation ONLY: a
    `Failed`->`Recomputed` (or `Blocked`->`Recomputed`) status
    transition propagates to blocked dependents and recovery
    subscribers regardless of value equality — see STATUS EVENTS
    in the joins bullet below.
  - **Recomputation is an eager topological pass, not demand-driven
    (concrete structure chosen per the authority's delegation).**
    The propagation above runs at the update point on the evaluator
    thread, coalesced LATEST-WINS PER TICK for controller-rate
    `upd` streams (the same coalescing as `set-tweak`), and rebuilt
    pattern/slot binds land through the standard dry-run + pending
    swap at the cycle boundary. Rationale: the scheduler already
    PULLS patterns per cycle; live-set form counts are small; and
    the eligibility/transaction/topological machinery above is
    accepted and sufficient — the Salsa/Adapton memoized
    demand-driven store remains a compatible later optimization
    behind the same `DepGraph` API, not a v1 requirement (the
    authority marks the structure "Recommended", the behavior
    "Decided"). Dirty-marking with deferred pull would delay
    recomputation to an unpredictable demand time mid-performance;
    eager-with-cutoff keeps latency bounded at the update.
  - **Dependency-set replacement and STALE-READ VALIDATION (rounds
    — the once-per-pass claim is QUALIFIED to once per round).**
    After each recomputation the form's edge set is REPLACED by its
    newly recorded eager reads (topology changes are first-class,
    not an anomaly). Within a pass, every slot write bumps its
    version, and every recomputation records the VERSION of each
    slot it read. A ROUND processes the scheduled set in
    topological order over the CURRENT edges; at round end, any
    form that read a slot which was RECOMPUTED LATER in the same
    round (recorded read version < the slot's current in-pass
    version — possible exactly when edges changed under it) is
    STALE and re-scheduled into the next round, ordered over the
    UPDATED edge sets; its dependents over updated edges are
    dirty-marked as usual. Rounds repeat until no stale read
    remains. TERMINATION AND CYCLES: over a fixed acyclic edge set
    a round is stale-free, so total re-executions are bounded by
    the scheduled-set size; exceeding that bound, or a back-edge in
    the updated topological order, is a GENUINE CYCLE under the
    changed edges — the pass stops for that subgraph with a cycle
    diagnostic and the previous coherent values stand.
    DETERMINISTIC ORDER: within a round, scheduled forms that
    are incomparable under the current edges run in DEFINITION
    (document) order, and mid-pass dirty-marking appends a form
    to the running schedule after its marking ancestor — every
    trace below is therefore the one exact schedule, not one of
    several legal ones, and its regression is deterministic.
    REACHABILITY OF STALE READS under the deref-time dirty-read
    abort (joins bullet below): a form never reads a slot whose
    owner is scheduled but not yet recomputed — that deref
    ABORTS and re-schedules the reader after the owner — so a
    stale read arises in exactly one way: the read slot's owner
    was UNSCHEDULED (Clean) at read time and became dirty LATER
    in the same pass through another form's value change or
    edge replacement. Astra's changing-edge case resolves
    correctly under this policy: with `A = if switch then B
    else root` and `B = if switch then root else A+1`,
    coalescing `switch=true, root=20`, round 1 in the OLD order
    runs A first; A's deref of the scheduled-but-not-yet-
    recomputed B is a DIRTY-READ ABORT (no stale value is ever
    read, nothing commits), B commits 20, then A re-runs after
    B and commits 20 — the same final values with no stale
    intermediate state. PASS JOURNAL AND
    PASS-LEVEL STAGING (the lifetime of provisional state across
    rounds): within a reactive pass, form transactions commit
    namespace writes immediately (later forms must read them),
    but every such commit is PROVISIONAL. The FIRST in-pass write
    to any slot JOURNALS its pre-pass value, version, and owning
    `FormGen`; re-execution journals nothing further — the
    PRE-PASS state, not the previous round, is the restoration
    target. Read versions, recorded dependency sets, and pass
    states live for the whole pass; and EVERYTHING ELSE a form
    transaction would emit is STAGED AT PASS LEVEL, not only
    slot-bind intents: ownership/generation advancement,
    `SlotControl` revocations, control-cell updates, tweak-table
    refreshes, and the `bindings` batch. NO host-visible effect
    of any kind leaves the evaluator before its producing result
    is validated. A re-executed form's later round SUPERSEDES its
    earlier staged intents, so only final, validated values can
    ever reach a sink or subscriber. VALIDATION AND ROLLBACK
    (pass end, before anything is released — the predicate is
    FAILURE-SENSITIVE; the round-13 pre-pass-read exemption is
    WITHDRAWN, since Astra showed it validates a result derived
    from a producer that failed later in the same pass): a
    form's result is VALID iff its final state is `Recomputed`
    AND, for EVERY eager read it made, the read slot's OWNING
    form's final pass state is `Clean` or `Recomputed` — a
    pre-pass version does NOT validate a read whose owner ended
    the pass `Failed` or `Blocked` (a failed transaction writes
    no value and bumps no version, so version comparison alone
    can never catch this) — AND every in-pass version it
    consumed was written by a VALID form. Computed by marking
    `Failed`/`Blocked` forms invalid and propagating invalidity
    over the recorded reads, keyed by owner AND by version, to
    fixpoint. For every INVALID form — a failed retry after a
    successful provisional round is the canonical case, a
    reader invalidated by its producer's LATE failure is the
    other — the journal RESTORES each slot it wrote to the
    pre-pass value/version/owner, ALL of its staged intents,
    revocations, ownership advancements, and tweak-state changes
    FROM ANY ROUND are DISCARDED, and it reports `Failed` (the
    failing form itself) or `Blocked` on the failing origin (a
    form whose reads consumed a rolled-back provisional value or
    a failed owner's retained value).
    VALID forms — independent successful branches — commit and
    publish normally. THREE NORMATIVE TRACES, all derived by
    applying the dirty-read abort policy literally (the earlier
    claim that the A/B retry case commits a provisional A=1/3
    is WITHDRAWN — Astra showed that intermediate state is
    unreachable under the abort gate; the traces and their
    regressions below are the reachable replacements):
    (1) ABORT/RETRY FAILURE — no provisional commit exists.
    `A = if switch then 1/B else root`, `B = if switch then 0
    else A+1`, `root = 2`; initially `switch = false`, A=2,
    B=3, old order A before B. After `upd switch true`, A runs
    first; its deref of the scheduled B is a DIRTY-READ ABORT
    (nothing commits — A never holds 1/3); B commits 0; A
    re-runs after B and FAILS on 1/0. A's failing transaction
    committed nothing, so its previous committed 2 simply
    stands — no journal restore is needed and none occurs; the
    regression asserts exactly that: the abort, the absence of
    any intermediate A value or staged A intent, A `Failed`,
    and B=0 committing as the valid independent branch.
    (2) REACHABLE PROVISIONAL ROLLBACK — the case that actually
    exercises the journal. It is reachable precisely because
    the invalidating owner is UNSCHEDULED at read time (the one
    stale-read path left open by the abort gate). Three forms
    in definition order X, Z, Y over `var n 0`:
    `X = if n > 0 then 1/(12 - Y) else 7` (initially 7, edges
    {n}); `Z = if n > 0 then 5 else 3` (initially 3, edges
    {n}); `Y = Z + 7` (initially 10, edges {Z} — a write to `n`
    does NOT schedule Y). `upd n 1` schedules X and Z only.
    X runs first (definition order; X and Z are incomparable):
    its deref of Y finds Y's owner Clean and UNSCHEDULED, so it
    legally reads 10 and X PROVISIONALLY COMMITS 1/2, staging
    its slot intent; X's edge set becomes {n, Y}. Z commits 5;
    the value change dirty-marks Y (edge Y->Z); Y runs, commits
    12, and dirty-marks X over X's CURRENT edges. Round-end
    validation finds X's recorded read of Y STALE; X re-runs
    and FAILS on 1/(12-12). X is invalid: the journal RESTORES
    X's slot to the pre-pass 7 (value, version, owner), X's
    round-1 staged intent and every other staged effect of X
    from any round are REMOVED (the explicit invalidation rule
    for an intent whose replacement fails), X reports `Failed`,
    and Z=5 and Y=12 — valid independent branches — commit and
    publish.
    (3) LATE FAILURE AFTER A LEGAL PRE-PASS READ — the producer
    fails WITHOUT any value or version change, so neither stale
    reads nor value propagation can reach the reader; the
    failure-transition rule (joins bullet below) and the
    failure-sensitive predicate above catch it. Definition
    order `X = if n > 0 then Y + 1 else 7` (initially 7, edges
    {n}); `Z = if n > 0 then 0 else 1` (initially 1, edges
    {n}); `Y = 1/Z` (initially 1, edges {Z}). `upd n 1`
    schedules X and Z. X runs first, legally reads the
    UNSCHEDULED Y = 1, and PROVISIONALLY COMMITS 2 with a
    staged intent. Z commits 0 and dirty-marks Y; Y runs and
    FAILS on 1/0, retaining its previous value 1 and writing NO
    version. Y's `->Failed` transition RE-SCHEDULES X (an
    in-pass reader of Y); X's retry aborts at its deref of the
    now-`Failed` Y and X becomes `Blocked` on Y. Validation
    independently agrees (X's consumed read's owner ended
    `Failed`): the journal restores X to the pre-pass 7, X's
    staged intent is removed, X publishes `blocked-on: Y` with
    7, Y publishes `Failed` with its retained 1, and Z=0
    commits as the valid branch. X's provisional 2 appears
    nowhere. In ALL traces no provisional value, bind, or
    revocation is ever visible at a recording host or
    subscriber.
  - **Failure at joins: FAILED and BLOCKED decided AT THE ACTUAL
    DEREFERENCE (replacing both the earlier
    not-scheduled-from-the-failed-form rule and the revision-11
    pre-execution gate, which inspected the OLD eager-read set
    and so could never let a conditional form drop a failed
    dependency).** Each scheduled form carries a pass state:
    `Clean`, `Recomputed`, `Failed`, or `Blocked`. A dirty form
    ALWAYS RUNS, transactionally; correctness is checked at each
    eager `Deref` it actually performs: (a) dereferencing a slot
    whose owner is `Failed` or `Blocked` ABORTS the form's
    transaction (nothing commits) and marks the form `Blocked`
    on that origin — its previous committed values and playing
    bindings stand and the batch reports `blocked-on`; (b)
    dereferencing a slot whose owner is scheduled this pass but
    not yet recomputed is a DIRTY-READ abort — the form
    re-schedules after that owner, counted against the same
    round-termination bound as a stale read. Topological order
    over current edges remains the scheduling heuristic; the
    deref check is the correctness gate. Because blocking
    attaches to reads ACTUALLY PERFORMED, every consumed path
    blocks — Astra's fixed-edge diamond still holds: `left =
    1/root` fails at `root = 0` (its failing transaction commits
    nothing, so its previous committed 1 stands), `right = root
    + 1` commits independently, and `total = left + right`
    dereferences `left`, aborts, and is BLOCKED keeping its
    previous committed 3 — it can never mix a failed branch's
    retained value with a sibling's new one — while a form whose
    CURRENT evaluation never touches a failed slot is free to
    complete. DEPENDENCY REDISCOVERY UNDER FAILURE follows
    directly, discovering the new read set without consuming
    failed values: `root = 1`, `switch = true`, `broken =
    1/root`, `selected = if switch then broken else 7`. Setting
    `root = 0` fails `broken` and blocks `selected` at its deref
    of `broken`. Setting `switch = false` triggers `selected`
    (writes to `switch` reach it through its recorded eager-read
    set, which still contains `switch`); it re-runs, its
    selected branch never dereferences `broken`, it commits 7,
    its edge set is REPLACED by the newly recorded reads
    ({`switch`}), and its `Blocked` state clears — with `broken`
    still `Failed` and unrepaired. METADATA ACROSS ANY
    UNSUCCESSFUL EVALUATION (what survives, what the attempt
    contributes — the round-14 scoping of this rule to aborts
    only is WITHDRAWN; Astra showed a form failing through an
    ORDINARY runtime fault after successful reads then had no
    wake-up path through those reads): EVERY evaluation that
    does not commit — a failed-deref abort, a dirty-read abort,
    OR an ordinary runtime failure (arithmetic, type, no-match,
    fuel, depth) — discards its transaction, writes, and
    staged effects, but NOT the form's durable bookkeeping.
    The last successful commit's values, committed edge set,
    and read versions all survive and keep serving ordinary
    value-triggered dirty-marking; dependency-set REPLACEMENT
    applies only on successful recomputation, never to an
    unsuccessful attempt. The unsuccessful attempt contributes
    exactly THREE records, and nothing host-visible: the pass
    state (`Blocked` with its failing origin for an abort;
    `Failed` with its unwind origin for an ordinary fault); a
    RECOVERY SUBSCRIPTION on every `Failed`/`Blocked` slot the
    attempt actually dereferenced (its blocking-read set —
    EMPTY for an ordinary fault whose reads were all healthy);
    and the ATTEMPT EDGE SET — every slot the attempt
    SUCCESSFULLY dereferenced before the failure point,
    whatever the failure kind. The wake-up dependencies of a
    `Failed` OR `Blocked` form are the UNION of its last
    successful committed edges, its attempt edge set, and its
    recovery subscriptions: a write to ANY of them
    re-schedules it, because a changed operand or selector
    anywhere on the attempted path can change the failing
    execution. Retaining these dependencies publishes nothing:
    the attempt's writes and effects are discarded regardless,
    the indexes are evaluator-internal, and they survive
    journal rollback (rollback restores slot values, not
    bookkeeping). Lifetime: the attempt edge set and
    subscriptions are REPLACED by each new attempt (whatever
    it reads and blocks or fails on) and CLEARED when the form
    next commits, at which point the new committed edge set
    takes over and the failure metadata is dropped. Recovery
    subscriptions and the attempt edge set are SEPARATE
    indexes from committed edges precisely because an
    ATTEMPTED read may involve slots absent from the last
    successful edges — switching TOWARD a failed dependency:
    with `broken` already `Failed`, `selected`'s last
    successful edge set is {`switch`} (it committed 7 without
    touching `broken`); `upd switch true` re-runs `selected`,
    which aborts at its newly discovered deref of `broken` and
    becomes `Blocked` with a recovery subscription on
    `broken`. Repairing `broken` MUST wake `selected` even
    though `broken` appears in no committed edge set of
    `selected` — the wake routes through the subscription.
    NEWLY DISCOVERED SELECTOR: with `broken` already `Failed`,
    `a = false`, `b = true`, and `selected = if a then (if b
    then broken else 7) else 5` committed as 5 over edges {a}:
    `upd a true` runs `selected`, which successfully reads `a`
    and `b`, then aborts on `broken` — its attempt edge set is
    {a, b} and its subscription is {broken}. `upd b false`
    writes a slot in the attempt edge set, so `selected`
    RE-RUNS, avoids `broken`, commits 7, and clears its
    `Blocked` state — with `broken` still unrepaired. Under
    the withdrawn two-record rule `b` was in no index and this
    recovery was unreachable. ORDINARY-FAILURE RECOVERY (the
    case that forced the abort-only scoping's withdrawal):
    `var a false`, `var b 0`, `selected = if a then 1/b else
    5` committed as 5 over edges {a}. `upd a true` runs
    `selected`, which SUCCESSFULLY dereferences both `a` and
    `b` — no owner is Failed, Blocked, or dirty — and then
    FAILS on division by zero. `selected` is `Failed` with its
    retained 5, its attempt edge set is {a, b}, and its
    blocking-read set is EMPTY (no subscription exists — every
    read's owner stayed healthy). `upd b 1` writes a slot in
    the attempt edge set, so `selected` RE-RUNS automatically,
    computes 1, commits, clears its `Failed` state and
    diagnostic, and replaces its committed edges with {a, b} —
    with no manual re-evaluation and no change to `a`. Under
    the abort-only scoping `b` was in no index and `selected`
    stayed permanently stale.
    STATUS EVENTS IN BOTH DIRECTIONS: a form's transition INTO
    `Failed`/`Blocked` and its transition OUT of them are BOTH
    status events that BYPASS the equality early-cutoff — a
    failed transaction writes no value and a repair may restore
    an equal one, so value propagation alone can carry neither
    direction. The FAILURE direction (the invalidation event,
    new this revision): when a form becomes `Failed` or
    `Blocked`, that transition (a) re-schedules, WITHIN the
    pass, every form whose recorded IN-PASS read set includes a
    slot the failing form owns — the reader's retry aborts at
    the deref-time `Failed` check, blocks, and any provisional
    commit it made is journal-restored at validation (trace 3
    above) — and (b) dirty-marks dependents over current
    committed edges ACROSS passes, so a dependent that no value
    change would ever reach still re-runs, hits the deref, and
    surfaces its `blocked-on` badge instead of silently keeping
    a value derived from the failed input. The RECOVERY
    direction: `Failed`/`Blocked` states are sticky across
    passes with their blocking-read sets; when a later trigger
    (or a manual re-eval) successfully recommits a failed form,
    that `Failed`->`Recomputed` transition dirty-marks BOTH the
    dependents over current committed edges AND every recovery
    subscriber of that slot. The
    bypass matters exactly when repair restores an identical
    value: `broken = 1/root` with `root` updated 1 -> 0 -> 1
    fails and then recovers to its retained pre-failure value
    1; value equality would stop propagation, but the status
    event still re-runs the blocked dependents and subscribers,
    so `total` (and a `selected` blocked TOWARD `broken`)
    retries, commits, and clears its badge. Each re-run that
    completes without a failed deref clears its state — under
    the unchanged ownership checks, one-shot exclusion,
    transactional staging, pass journal, and boundary
    activation.
  - **Publication — subscribers see one coherent step.** When the
    pass completes (all rounds done; every scheduled form
    `Recomputed`, `Failed`, or `Blocked`) AND validation/rollback
    has run — so a rolled-back provisional value can appear in no
    batch — the session publishes
    ONE `bindings` batch on the protocol (14.4): the changed
    top-level names with display values, refreshed tweak-site
    tables for rebuilt forms, and PER-FORM STATES — failure
    diagnostics with origin and `blocked-on` markers. Editor
    displays and sliders subscribe and repaint from the batch
    (push notification, pull-on-wake); slots activate via the
    pass-level staged intents promoted to their pending swap.
    Nothing is published mid-pass, so a display can never show a
    partially updated or stale-round set; a `Failed` form's and a
    `Blocked` form's previous committed values stay visible (with
    their state badges) per the joins rule above, keeping the
    live state coherent under failure.
  - **Static edges from the checker (advisory).** `types/deps.rs`
    computes each top-level form's free-variable edge set, split
    eager/late using the same analysis that derives forcing masks
    (5.5), for LSP hover ("depends on: root") and subscription
    precomputation. The RUNTIME recording above is authoritative
    where they differ (conditional reads); the checker set is a
    display/optimization input, per the authority's "static edges
    from the checker" recommendation.
  - **Authority question (recorded, not silently resolved).** The
    lang-reference reactive example writes `let base 60` then
    `upd base 62`, conflicting with the standing immutable-`let` /
    `upd`-on-`var` rule. This ships as an authority-question
    fixture (plan Verification); the design implements the
    unambiguous reading — `upd` targets a `var`; a `let` changes by
    redefinition — and BOTH triggers propagate identically, so no
    behavior hangs on the answer.
  This is live redefinition automated with guard rails, not a new
  semantics: an eligible rebuild does exactly what re-evaluating that
  line by hand does — now for EVERY eager upstream change, `var`
  updates included: `var root 60`, `let raised + root 7`,
  `s :pluck > note [raised] > d1`, then `upd root 62` recomputes
  `raised` (eager edge on `root`), rebuilds the pattern form that
  eagerly read `raised`, re-binds `d1` at the boundary, and updates
  the display batch — nothing else recomputes.

**Scope model (20 Q1, Decided 2026-09-25; lang-reference section 4).**
Scopes form the chain `prelude -> session -> fn/block`.
- The PRELUDE is a read-only table of natives and prelude values
  (including `default-sound-kit` and `sound-kit`, 7.1.4). No form can
  write it: a session `let`/`var`/`fn` of a prelude name creates a
  SESSION binding that shadows it, and `upd` of a prelude name is
  `upd-immutable`.
- The SESSION scope holds the top-level bindings of the editor buffer
  or REPL. A file read by `load` gets its OWN fresh session-level scope
  whose parent is the prelude, never the caller's session (7.1.3).
- A child scope is opened by a `fn` (its parameters and the top-level
  statements of its body share one scope), a lambda (parameters and
  body), a `{..}` or indented block, and a `match` clause (its pattern
  bindings and guard). Expander output keeps these boundaries: `if`
  becomes `match`, `for` becomes `map` over a lambda.
- Name resolution walks the chain innermost first; for the session,
  open imports sit between the session and the prelude (5.7).
  Resolution happens at compile time and fixes the slot a form reads:
  a later session binding of a prelude name does not retarget forms
  compiled before it (it is a new binding, not a write to the prelude
  slot). The one dynamic lookup is `sound-kit`, which `s` reads by name
  at every query (7.1.4).
- Diagnostics (checker only, 7.1.4): a second binding of a name in the
  same scope is `rebinding` (error); a binding that shadows a prelude
  name is `shadows-prelude` (hint); a binding that shadows a user
  binding of an enclosing scope is `shadowing` (warning). In Live mode,
  re-evaluating a top-level definition is redefinition, never
  `rebinding`, so the runtime `Namespace` never rejects a session write
  for this reason; `rebinding` is found only by checking a whole
  document or spec block.

### 5.7 Packages, imports, and qualified names (Decided scope)

Implements the Decided package system (architecture.md Packages;
lang-reference.md modules): Go-style repository-path imports, git-tag
semver versions, minimal version selection, a lock file, a cache, and
a browser proxy. Packages are Vactr code plus assets only — no
native extensions — so a package can never reach the audio thread
except through builtins.

```rust
pub struct PackageId(Rc<str>);         // "github.com/owner/name", lowercase path
pub struct PkgManifest {               // vactr.toml — fields DEFINED here, as the
    package: PackageId,                // spec delegates them to this design
    vactr: Option<Rc<str>>,          // minimum language version (semver)
    deps: Vec<(PackageId, Rc<str>)>,   // path -> version requirement ("v1.2.0"-style tags)
    assets: Vec<Rc<str>> }             // sample/wavetable directories, relative
pub struct LockEntry { id: PackageId, version: Rc<str>, sha256: [u8; 32] }
pub struct LockFile { entries: Vec<LockEntry> }   // vactr.lock, sorted by path
pub trait PackageStore {               // host capability — IO side, never evaluator-blocking
    fn resolve(&mut self, roots: &[(PackageId, Rc<str>)]) -> Result<Vec<LockEntry>, HostError>;
    fn fetch(&mut self, e: &LockEntry) -> Result<PkgSources, HostError>; }
pub struct PkgNs { id: PackageId, ns: Namespace }     // one child namespace per package
pub struct ImportBinding { prefix: SymId, pkg: PackageId, open: bool }
```

- **Resolution.** `vactr get` (CLI) and the editor's import action
  run MINIMAL VERSION SELECTION exactly as Go: collect every
  requirement reachable from the roots, pick the MAXIMUM of the
  MINIMUM required versions per path, write `vactr.lock` with the
  chosen version and the CANONICAL CONTENT DIGEST below.
  Native `PackageStore`: shallow git fetch of the tag (or a local
  directory), cached under `~/.vactr/pkg/<path>@<version>`.
  Browser `PackageStore`: HTTPS to a package proxy, Go-proxy-shaped
  (`{proxy}/{path}/@v/list`, `{proxy}/{path}/@v/{version}.zip`), same
  cache semantics in IndexedDB/OPFS. Either way the fetched content
  digest MUST match the lock entry; a mismatch is an integrity
  diagnostic and the package does not load.
- **Canonical content digest — archive-format-independent AND
  injective (the contract shared by git, local-directory, and
  proxy stores).** The lock digest is computed over the package's
  FILE TREE, never over an archive's bytes (git checkouts and
  proxy zips differ in metadata; hashing an archive is not
  portable). The serialization is LENGTH-PREFIXED, not
  delimiter-based — the newline-record form of the previous
  revision is WITHDRAWN (Astra demonstrated two distinct trees
  with identical newline-serialized bytes via a filename
  containing an embedded newline): for every regular file under
  the package root, in bytewise-sorted order of its normalized
  relative path (forward slashes, UTF-8), emit the binary record
  `u32_be(len(path_bytes)) || path_bytes || sha256(contents)`
  (the content hash as 32 raw bytes); the lock digest is the
  sha256 of the concatenated records. Length prefixes make the
  encoding INJECTIVE over all byte-string paths — no filename can
  forge a record boundary — and, independently, path VALIDATION
  (below) rejects control characters outright, so the demonstrated
  counterexample is excluded twice: its tree fails validation, and
  even hypothetically its canonical bytes differ. A git checkout,
  a local directory, and an extracted proxy zip of the same
  sources produce the SAME digest by construction; every store
  applies the same serialization and validation before hashing and
  publication. Only regular files participate; anything else fails
  validation below before hashing.
- **Extraction safety and containment (a matching hash never makes
  unsafe content safe — validation runs FIRST, on every store).**
  Every entry path must be relative, forward-slash, with no empty,
  `.`, or `..` segments and no drive/absolute prefix; it must be
  valid UTF-8 containing NO control characters (any byte below
  0x20 — including newline, the digest-forgery vector — and 0x7f
  are rejected); entries that
  are symlinks, hardlinks, or devices are REJECTED; duplicate
  entries — including case-fold duplicates, for case-insensitive
  filesystems — are REJECTED; entry count and total uncompressed
  size are bounded (tier-configurable caps, checked DURING
  extraction, abort on exceed). Manifest `assets` paths are
  normalized and must resolve strictly UNDER the package root, or
  the asset is not registered (diagnostic). Any violation is an
  integrity diagnostic naming the entry: the package does not load,
  even when its digest matches the lock.
- **Atomic verified cache publication.** Fetch/extract into a
  staging location (temp dir; OPFS staging dir in the browser),
  validate as above, compute the canonical digest, compare with the
  lock, and only then ATOMICALLY move the tree into the cache path
  and write a digest stamp used for cheap verified reads. A crash
  or failure at any earlier step leaves NO partial cache entry; the
  cache never holds unverified content.
- **Failure behavior.** Unresolvable version, network failure, hash
  mismatch, manifest parse error, or a package's own compile
  diagnostics: the import fails with a load-time diagnostic carrying
  the package path; the SESSION CONTINUES without the package (a live
  session never dies), and qualified names under that prefix are
  undefined-name diagnostics. Fetching happens on the IO/editor side
  through `PackageStore`; the evaluator receives already-fetched
  sources and never blocks on the network; nothing package-related
  touches the audio thread.
- **Loading.** Import (evaluator thread) reads the fetched sources
  through the SAME read -> expand -> check -> compile pipeline into
  the package's `PkgNs`; package diagnostics are attributed to the
  package's files (their own `FileId`s). Package assets register with
  `SampleLoader` (sample banks, wavetables) under package-qualified
  keywords. Package top-level forms are ordinary forms; their slots
  live in `PkgNs` and are late-bound like any other, but a package is
  not live-edited: re-import replaces the whole `PkgNs`.
- **Names, and SAME-FILE frontend ordering.** `import path` binds the
  default prefix (the last path segment minus a `vactr-` prefix:
  `pads`); `as pd` binds an alias; `open` additionally splices the
  package's public names into the session's OPEN-IMPORT list.
  Qualified name `pads.warm` is `identifier "." identifier`,
  accepted only under a bound prefix — and the binding environment
  is a DOCUMENT-LOCAL `AliasEnv`, not a frozen session snapshot,
  because the authoritative example puts `import` and `pads.warm` in
  the SAME file. The frontend is TWO-PHASE per document:
  - Phase 1 (syntactic prescan): the reader splits the document into
    top-level forms by layout alone and recognizes `import` forms
    purely syntactically (`import` is a fixed leading keyword — no
    alias context needed). This yields the document's import list
    without evaluating anything.
  - Phase 2 (form-by-form read): each form is read against
    `AliasEnv` = session aliases + the imports declared in EARLIER
    forms of this document, threaded form by form — so
    `import ... vactr-pads` on line 1 makes `pads.warm` readable
    on line 2 of the same fresh-session file. An import whose
    package failed to load leaves its prefix bound-but-broken:
    qualified uses read fine and become load-diagnostic references,
    not reader errors, and the session continues.
  Fetching stays off the evaluator: for an eval of a whole document
  the session hands the phase-1 import list to `PackageStore`
  (IO side, including transitive dependency imports discovered from
  fetched manifests) and evaluates forms in order once sources are
  available; a single-form eval inside an editor buffer uses the
  buffer's phase-1 scan for its `AliasEnv`. The LSP builds the same
  document-local `AliasEnv` from its OWN phase-1 scan WITHOUT
  executing anything; package interface data for typing comes from
  the lock/cache via `PackageStore` (an unfetched package types its
  qualified names as `any` with a "package not fetched" diagnostic
  rather than failing analysis). Lookup order: locals > session
  namespace > open imports (most recent import wins; any collision
  among opens or with the prelude is an LSP warning per the spec,
  and the qualified spelling always remains available) > prelude.
  The per-scope rule (5.6 scope model, 20 Q1) governs the session
  namespace itself; open-import precedence is the spec's own
  collision-warning model (`import-collision`), not shadowing inside a
  scope.
- **Checker/LSP.** `HostManifest` gains the per-package keyword sets
  from assets; qualified names type through the package's `PkgNs`;
  completion offers prefixes, qualified names, and open names.

## 6. Reader and Expander

### 6.1 Source model

```rust
pub struct Span { file: FileId, start: u32, end: u32 }     // byte offsets
pub struct SrcRef { span: Span, doc_revision: u64, form_gen: FormGen } // 5.6, 14.4
pub struct Node { id: NodeId, kind: NodeKind, span: Span, children: Box<[Node]> }
pub enum NodeKind { Call, List, Block, Pair, Atom(Atom), … }
```

`NodeId` is stable per (file, position) within one read; spans survive
through expansion, checking, compiling, pattern construction, and
scheduling so that every diagnostic and every telemetry event can point
back at source (section 14). No stage below the reader sees indentation.

### 6.2 Lexer

Tokens per the decided identifier rule
`[a-zA-Z][a-zA-Z0-9]*(-[a-zA-Z0-9]+)*` (a `-` inside a name must be
followed by a name character, so `a->b` lexes as three tokens; `-x` is
negation, `- x` the function), keywords `:name`, the operator class
(`+ - * / = < > <= >= .. -> & | ?`, not user-definable), number
literals (int/float/ratio `1/4`), strings with `{}` interpolation
(the lexer nests a sub-expression scan), `_` wildcard, `_<digits>`
console registers, tabs as the only indentation, `( )` and stray tokens
as reader errors. The `import` form (`import <repository-path> [as
alias] [open]`) and QUALIFIED NAMES (`identifier "." identifier`) are
read here via the TWO-PHASE frontend of 5.7: phase 1 splits top-level
forms and recognizes `import` forms syntactically; phase 2 reads each
form against the document-local `AliasEnv` (session aliases + earlier
same-document imports), so import-then-use in one fresh file reads
correctly; a qualified name whose prefix is bound by NO session or
earlier-form import remains the spec's reader error. Directive
comments (`#@ ...`) are TRIVIA to the language, kept with positions
like every comment; their interpretation is the editor layer's
(section 13.5).

### 6.3 Layout rules (decided piecemeal in lang-reference.md)

- One line is one prefix call; `let name` reads the rest of its line.
- A trailing `:` opens an indented block — the multi-line spelling of a
  `{}` group; one tab per level; the formatter normalizes.
- `{}` is the only inline nesting form; one expression per inline `{}`.
- A line beginning with `>` continues the previous expression (pipe:
  left value becomes the FIRST argument of the call on the right).
  `>` in head position of a line or `{}` group is greater-than.
- `name: value` on one line is a pair; `name:` at end of line opens a
  block; `slot :drums gain: 0.8:` is therefore legal and the reader
  resolves it by position.
- `_1` outside the console is a reader error.

The reader is lossless enough for the formatter and the LSP: it keeps
comment and trivia spans in a side table, and it recovers from errors
(a bad line yields an `Error` node; following lines still parse) so the
editor gets diagnostics while typing.

The formatter rules (which lines are re-indented, whitespace policy,
directive-preserving comment indentation, the reader gate) are in
`design-formatter-and-syntax.md` section 3.

### 6.4 Expander

Compile-time only, fixed sugar (no user macros in v1): `if`/`elif` to
`match` (the decided desugaring is the single place truthiness is
defined), binding `if` to a two-clause `match`, `for` to `map` with the
result discarded, pairs at call sites to trailing pair arguments,
string interpolation to a concat call, `x ? d` to `or x d`, `-x` to
`{neg x}`, pipe lines already resolved by the reader. Output is kernel
forms only: `match fn let var upd -> {} enum` plus calls — the decided
kernel. The expander is a pure `Node -> Node` pass; spans are preserved
(a synthesized node carries its origin span).

### 6.5 Front-end implementation decisions (TASK-001..003, 2026-09-25)

These decisions were recorded when TASK-001..003 started. They fill in
details that sections 4-6.4 leave open, and they change no Decided item.
The questions they raised (U1-U5 in
`design-docs/user-qa/pending-frontend-questions.md`) were all answered
on 2026-09-25. Each answer is marked **(U<n>, answered)** where it
applies, and it is binding for TASK-001..003.

#### 6.5.1 Foundation types and placeholder shells

| Type | Path | Later owner |
|------|------|-------------|
| `Span`, `FileId` (`FileId::CONSOLE` = id 0; the plan's "`FileId::Console`"), `NodeId`, `SrcRef` | `src/reader/span.rs` | final |
| `KwId`, `SymId`, `Interner` | `src/value/intern.rs` | final |
| `Failure`, `FailCode`, `Origin` (8.4) | `src/vm/fail.rs` | final; TASK-005 adds codes |
| `Diagnostic`, `Severity`, `DiagCode`, `RunOrigin` (section 7) | `src/types/diag.rs` | final; TASK-004 adds codes |
| `FormGen(u64)`, `VarSlotRef` (shell; derives `Clone` and `Debug`, since `Value` derives `Clone` and holds it by value) | `src/ns/namespace.rs` | TASK-005 |
| `TweakId` | `src/ns/tweak.rs` | TASK-005 |
| `Closure` (shell) | `src/compile/proto.rs` | TASK-005 |
| `Pat` (shell), `Sig` (shell) | `src/pattern/pat.rs`, `src/pattern/signal.rs` | TASK-006 |
| `TexNode` (shell), `OutId` | `src/tex/texnode.rs` | TASK-006 |
| `SlotId`, `CtlId(u16)` | `src/sched/slots.rs` | TASK-007 |
| `InstId` | `src/dsp/graph.rs` | TASK-008 |

- Id newtypes wrap `u32`, except `FormGen` (`u64`) and `CtlId` (`u16`, per
  11.4). Each derives `Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash,
  Debug` and has `new` and `get`. Nothing else.
- A shell is a struct with one private unit field and `Debug`. It has no
  methods and no constructor. No TASK-001..003 code or test builds a
  `Value` that holds a shell, so later tasks may replace a shell
  without keeping any behavior. `src/lib.rs` declares only the modules
  that contain these files: `value reader expand types ns compile vm
  pattern tex sched dsp`. `clock`, `host`, `session` and `lsp` wait for
  their tasks.
- The other types a `Value` needs are defined in `src/value/value.rs`:
  `StructVal { ty: SymId, fields: Box<[(KwId, Value)]> }` and
  `VariantVal { enum_ty: SymId, tag: SymId, fields: Box<[(KwId, Value)]> }`
  (both with fields sorted by key name, 5.4), `NativeId(u32)`, and
  `RangeVal { start: i64, end: Option<i64> }` (exclusive end;
  `None` means the lazy open range).
- Cargo features: `default = ["host-native"]`; `host-native`, `host-wasm`
  and `lsp` start empty. No dependency is added, including dev-dependencies.
- `src/main.rs` prints only `vactr <version>`, because the placeholder
  `hello()` it calls is removed. The CLI belongs to TASK-009.

#### 6.5.2 Interner scope

`Key` must be `Ord` without a context argument, because it is a
`BTreeMap` key, and keyword order is alphabetical (5.2). So the
`Interner` lives in a thread-local on the evaluator thread, matching 5.2
("evaluator-thread-only"). The free functions `intern_kw`, `intern_sym`
and `name_of` reach it, and `Ord for Key` resolves keyword names through
it. A `KwId` or `SymId` has no meaning on another thread. That is
already the invariant for `Value`, and the POD handoff (11.4) converts
ids before anything crosses threads.

#### 6.5.3 Numbers, keys, equality, accessors, print (TASK-001)

- **Widening lattice** (`src/value/num.rs`; lang-reference section 2).
  `NumKind = Int | Int64 | Float | Float64 | Ratio`. `join(a, b)` is
  symmetric:

  | join | Int | Int64 | Float | Float64 | Ratio |
  |------|-----|-------|-------|---------|-------|
  | Int | Int | Int64 | Float | Float64 | Ratio |
  | Int64 | | Int64 | Float64 | Float64 | Ratio |
  | Float | | | Float | Float64 | Float |
  | Float64 | | | | Float64 | Float64 |
  | Ratio | | | | | Ratio |

  `widen(&Value, NumKind) -> Result<Value, Failure>` converts only
  upward along the lattice. Any narrowing request is
  `Failure(FailCode::Type)`, because narrowing is only the explicit
  `int`, `int64` and `round` calls. The join governs `+ - *` and the
  comparisons. `/` of two exact operands producing a ratio is TASK-005
  arithmetic.
- **Ratio64** is as in 5.3. It uses `i128` intermediates, and a
  normalized result that does not fit in `i64` is
  `Failure(FailCode::Overflow)`. A zero denominator is
  `Failure(FailCode::DivisionByZero)`. `Display` prints an integral
  ratio as an int (`1`) and any other ratio as `3/8`.
  `from_decimal(&str) -> Option<Ratio64>` gives the exact value of a
  decimal literal (`1.5` becomes `3/2`), or `None` on overflow.
- **NumKey** (`src/value/key.rs`) has two forms. `Exact(Ratio64)` holds
  every int, int64 and ratio, and also every finite float whose value is
  exactly a `Ratio64` (a dyadic rational in range). `Float(f64)` holds
  the other finite floats. Keys order numerically by EXACT value
  (an `Exact` against a `Float` is compared without rounding, via
  i128 shifts); because a `Float` key is never exactly a `Ratio64`,
  an `Exact` and a `Float` are never equal, and the order is total
  without a tie rule. (Amended 2026-09-25 from "equal as f64, Exact
  first": that rule is total only under a monotone ratio-to-f64
  conversion, which `to_f64` does not guarantee; the implementation's
  exact comparison is adopted.) `-0.0` normalizes to `0`. A NaN key, or a
  key that is not a number, keyword or string, is
  `Failure(FailCode::Type)` (5.4). So `1`, `1.0` and `1/1` are the same
  key.
- **deep_eq** (`src/value/eq.rs`) follows the key normalization:
  - Numbers compare by exact value across widths, so `= 1 1.0` is true.
    NaN is not equal to itself.
  - `Nil`, `Bool`, `Keyword` and `Str` compare by value.
  - A `List` compares elementwise.
  - A `Dict` equals a `Dict` with the same entries. A `Dict` equals a
    `List` exactly when the list's elements are the dict's pairs in key
    order. This follows from "a dict is a list of pairs", so `[]` as an
    empty list equals `[]` as an empty dict.
  - A `Struct` equals only a struct of the same type with equal fields.
    A `Variant` compares tag and fields.
  - `Inst` compares by id, and `Range` compares structurally.
  - `Fn`, `Native`, `Thunk`, `VarRef`, `Pattern`, `Signal` and `Tex` give
    `Err(Failure(FailCode::Type))`. The caller has already forced a
    thunk or dereferenced a `VarRef` (5.1).
- **Accessors** (`src/value/access.rs`):
  - `truthy`: only `Nil` and `Bool(false)` are falsy.
  - `index(v, i)` is 0-based. `Nil` gives `Nil`. An index out of range,
    including a negative one, gives `Nil`. A non-integer index is
    `Failure(Type)`.
  - `get(v, key)`: `Nil` gives `Nil`. A dict miss gives `Nil`. An
    unknown struct or variant field is `Failure(FailCode::UnknownField)`.
  - `len(v)`: `Nil` gives 0.
  - `first(v)`: `Nil` or an empty collection gives `Nil`. A dict gives
    its smallest pair.

  Only accessors pun nil. `put` on `Nil` is `Failure(Type)`, following
  lang-reference ("arithmetic and calls do not").
- **Dict operations** (`src/value/dict.rs`):
  - `dict_from_pairs`: a repeated key keeps the last pair.
  - `put(coll, elems)`: on a list it appends. On a dict every element
    must be a pair (a 2-list with a key-typed head); otherwise it is
    `Failure(Type)`. Later pairs win.
  - `join(colls)`: the first collection sets the result kind. A dict
    merges the others, which must be dicts or lists of pairs. A list
    concatenates the others, and a dict among them contributes its
    pairs. An empty input gives the empty list.
  - `pairs(d)`: iterates in key order.
- **Print format** (`src/value/print.rs`, `Display for Value`).
  Values print in literal syntax:
  - `nil`, `true`, `false`.
  - Ints as written.
  - Floats use Rust's shortest round-trip form for their width, with
    `.0` appended when the output has no `.` or exponent (`90.0`).
  - Ratios print per `Ratio64` above.
  - Keywords print as `:name`.
  - A string prints raw at top level and quoted, with escapes, inside a
    collection.
  - Lists print as `[..]`.
  - A dict prints with pair sugar when the key is a keyword
    (`[amp: 0.5 pan: -1]`) and as `[k v]` otherwise.
  - A struct prints as `name field: v ..`, and a variant as `tag v ..`.
  - A range prints as `0..8` or `0..`.
  - Opaque values print as `<fn>`, `<native>`, `<thunk>`, `<var>`,
    `<pattern>`, `<signal>`, `<inst>` or `<tex>`.

#### 6.5.4 Reader (TASK-002)

Files in `src/reader/`:

| File | Contents |
|------|----------|
| `span.rs` | span and id types |
| `node.rs` | `Node`, `NodeKind`, `Atom`, `Op`, `Trivia` |
| `lexer.rs` | tokens, including string interpolation |
| `layout.rs` | physical lines to statements |
| `line.rs` | one statement's tokens to a `Node` |
| `import.rs` | `ImportDecl`, `AliasEnv`, `prescan_imports` |
| `sexpr.rs` | canonical printer |
| `mod.rs` | `read`, `ReadResult` |

**Node model.** `Node { id, kind, span, children }` as 6.1 defines it.

`NodeKind` has these variants:

| Variant | Children |
|---------|----------|
| `Atom(Atom)` | none |
| `Call` | `[head, args..]` |
| `List` | items; a dict literal is a list of pairs |
| `Block` | statements |
| `Pair` | `[key, value]` |
| `Arrow` | `[lhs.., body]`; the body is always the last child and the lhs may be empty |
| `Splat` | `[expr]` |
| `Neg` | `[expr]` |
| `Fallback` | `[lhs, rhs]` |
| `Interp` | `[parts..]`, each part a `Str` atom or an expression |
| `IfChain` | `[if-stmt, elif-stmt.., else-stmt?]` |
| `Import(Box<ImportDecl>)` | none |
| `Error` | none |

`Atom` has these variants:
- `Int(i64)`. Kind resolution waits for the checker (5.3).
- `Float { value: f64, exact: Option<Ratio64> }`.
- `Ratio(Ratio64)`.
- `Str(Rc<str>)`, `Keyword(Rc<str>)`, `Sym(Rc<str>)`.
- `Qualified { prefix, name }`.
- `Op(Op)`, where `Op` is one of `+ - * / = < > <= >= .. -> & | ?`.
- `Wildcard`, `ConsoleReg(u32)`, `Nil`, `Bool(bool)`.
- `Builtin(Rc<str>)`, which only the expander produces (6.5.5).

The reader stores names as text, not interned ids. It never touches
the interner or `Value`.

Node ids are sequential in pre-order across one `read`, starting at 0,
and `ReadResult::next_node_id()` returns the first free id. Spans are
byte offsets into the source.
- A CRLF line end counts as the terminator: `\r` belongs to no span.
- A tab is one byte.
- Non-ASCII text is allowed only inside strings and comments.
- A source longer than `u32::MAX - 1` bytes gets one diagnostic,
  `source-too-large`, and no nodes.

**Lexer rules.** These add to 6.2.
- **Numbers.** Allowed forms are `12`, `-3`, `2.5`, `-0.25`, `1/4` and
  `-1/4`. A float needs a digit after the `.`, so `0..8` is a range.
  A literal that does not fit in `i64` is `bad-number`. So is a ratio
  with a zero denominator, and so is a digit sequence followed
  directly by a name character (`1st`).
- **Negative literals and negation.** `-` directly followed by a digit is
  part of a negative literal. `-` directly followed by a name or a `{`
  is `Neg` when it comes at the start of a line or right after a space,
  `{` or `[`. Otherwise `-` is the operator.
- **Ranges.** `a..b` and `a..`, written without spaces, read as
  `Call[Op(..), a, b]` and `Call[Op(..), a]`.
- **Qualified names.** `x.y` with both parts identifiers is `Qualified`.
- **Names and underscores.** `_` alone is `Wildcard`. `_` followed by
  digits `[1-9][0-9]*` is `ConsoleReg`, and it is `console-register-in-file`
  unless the file is `FileId::CONSOLE`. Any other name containing `_`
  (`_tmp`, `my_name`), or a name ending in `-` (`foo-`), is
  `bad-identifier`.
- **Stray characters.** `( )` is `paren-form`. Any other character
  outside the token set (`! ; , @ $ ~`, non-ASCII) is `stray-char`.
  A `~` that starts a `~/` path is a path, not a stray character
  (6.5.8).
- **Paths and urls.** Unquoted path and url literals are lexed before
  the number, operator and name rules (6.5.8).
- **Comments.** `#` outside a string starts a comment that runs to the
  end of the line.
- **Strings.** A string stays on one line; an unfinished one is
  `unterminated-string`. The escapes are `\" \\ \n \t \{ \}`, and any
  other escape is `bad-escape` **(U3, answered)**. A `{` inside a
  string starts a nested expression that runs to its matching `}`.
  The nested scan follows the full token rules,
  so nested strings and groups are allowed. A string with at least one
  `{}` reads as `Interp`, and one without reads as a `Str` atom.
- **Colon classes.** The token right before a colon decides its role:
  - A colon with only whitespace or a comment after it on the line is
    the block opener. So `d1:`, `->:`, `0.8:` and `:drums:` each open a
    block.
  - A colon directly after a key token (identifier, string or number)
    and followed by whitespace and a value is a pair key. An identifier
    key becomes a keyword key: `amp: 0.5` reads as `[:amp 0.5]`.
  - `:name` at the start of a line, or right after whitespace or an
    opening `{` or `[`, is a keyword.
  - Any other colon is `misplaced-colon`.

**Layout** (`layout.rs`). Blank and comment-only lines affect only
trivia. Indentation counts leading tabs. A space inside the indentation
is `indent-space`, and that line reads as `Error`.

A body at level `L` (the top level is `L = 0`) holds statements at
exactly level `L`. When a line ends with a block colon, the lines after
it at level `>= B` form that block's body, where `B` is the opening
line's level + 1. A body with no lines is `empty-block`.

A line that begins with `>` and is deeper than `L` continues the most
recent statement of the body. Its tokens are appended to that
statement's tokens, so the `>` falls in mid-statement position and acts
as the pipe. The formatter normalizes such lines to `L + 1`. A
continuation line may itself end with a block colon. After a statement
has opened a block, a later continuation line is
`continuation-after-block`. Any other deeper line is `unexpected-indent`.

This reconciles the two Decided sentences of lang-reference section 1.
A line that begins with `>` continues the previous expression when it is
indented under it. A `>` at statement level (level exactly `L`) is in
head position, so it is greater-than. Every example in the spec fits
this reading **(U2, answered: confirmed; a fixture pins it)**.

After a body is split into statements, a statement headed `elif` or
`else` joins the statement just before it in an `IfChain` when that
statement is headed `if` or `elif` and no `else` has closed it. The
statement can be a `Call`, or an `Arrow` whose lhs starts with `if`. A
stray `elif` or `else` is left alone, and the expander reports it.

The unit of error recovery is the statement, including its
continuations and its block. A bad statement becomes one `Error` node
with the diagnostics recorded, and the next statements still read.

**Statement grammar** (`line.rs`). The statement's head token picks the
rule:

- `import` reads `import PATH [as ALIAS] [open]` and nothing else. It is
  allowed only at the top level; elsewhere it is `import-not-top-level`.
  - `PATH` is lowercase ASCII `[a-z0-9._-]` segments joined by `/`,
    with at least one `/`. No segment may be empty, `.` or `..`.
  - The prefix is `ALIAS`, or else the last segment with any `vactr-`
    prefix removed. It must match the identifier rule.
  - A violation is `bad-import`.
- `let`, `var` and `upd` read `Call[head, TARGET, EXPR]`. `TARGET` is
  exactly one item: a name, a `name: type` pair, or a `[..]` pattern.
  `EXPR` is the rest of the statement, including continuation lines,
  read by the expression rule. This implements "`let` reads the rest
  of its line", with pipes inside it. An empty rest is
  `binding-without-value`.
- `fn` reads the statement as a flat item list. A top-level `->` does
  not split it; it stays an `Op` atom, the return-type marker of
  lang-reference section 4.
- Any other head goes through the expression rule, in four steps:
  1. **Arrow.** Split at the first `->` outside brackets. The lhs is a
     flat item list; lhs items are patterns or header items and are
     never piped. The body is the expression rule applied to the rest.
     A missing body with no block is `arrow-without-body`.
  2. **Pipe.** Split the body at each `>` that is not the segment's
     first token.
  3. **Fallback.** Split each segment at `?` into operands. The fold
     puts the running value in as the first argument of each later
     segment's first operand. Then `?` folds left:
     `d :gain ? 1.0 > * 2` reads as
     `Call[*, Fallback(Call[d, :gain], 1.0), 2]`. An empty operand is
     `misplaced-fallback`.
  4. **Operand.** One item reads as itself; two or more read as a
     `Call`.
- A trailing block colon's `Block` becomes the last item of the last
  operand. So `slot :drums gain: 0.8:` reads as
  `Call[slot, :drums, Pair, Block]`, and `x ->:` reads as an `Arrow`
  whose body is the `Block`.

The item kinds:
- An atom.
- A pair: a key and exactly one item. A missing value is `bad-pair`.
- `& item`, which reads as `Splat`. A missing operand is
  `misplaced-splat`.
- `Neg`.
- A `[..]` list. It holds items only, so `->` or `?` inside it is a
  diagnostic, and `>` inside it is an `Op` atom. The implementation
  names the `->` case `misplaced-arrow`, and it reports a pipe `>`
  with no call after it (`print 1 >`) as `empty-pipe`.
- A `{..}` group. Its content goes through the expression rule, so a
  `>` at its start is in head position. A group whose content is an
  `Arrow` reads as that `Arrow`, with the group's span; this is the
  lambda. Any other group reads as `Block[expr]`. `{}` is
  `empty-group`. A group still open at the end of its line is
  `unclosed-group` (no multi-line `{}`).
- An unclosed `[` is `unclosed-bracket`.

A qualified name whose prefix is bound in no `AliasEnv` entry is
`unbound-qualifier`. Nesting of groups, lists, interpolation and
blocks is capped at depth 128; deeper input is `nesting-too-deep`.
The reader and expander must never panic on any input, because
packages and the editor feed them untrusted text.

**Imports** (`import.rs`).
- `ImportDecl { path, alias: Option<Rc<str>>, open: bool, prefix, span }`.
- `AliasEnv { prefixes: BTreeMap<Rc<str>, Rc<str>> }` maps a prefix to
  its package path.
- `prescan_imports` splits the text into top-level statements by layout
  alone and parses those headed `import`. It skips malformed imports,
  because `read` reports them.
- `read` clones the given env. After each top-level `import` form it
  adds that form's prefix, so later forms in the same document read
  `pads.warm` (5.7).
- `open` has no effect on the reader.

**Trivia.** `Trivia { items: Vec<TriviaItem { span, kind: Comment | Directive }> }`
lists every comment in source order, including trailing comments. A
span runs from `#` to the end of the line text. A comment whose text
starts with `#@` is a `Directive`. Attaching directives to statements
is left to the editor layer (13.5).

**Canonical printer** (`sexpr.rs`, `print(&Node) -> String`). The golden
tests use it.

| Node | Printed form |
|------|--------------|
| `Call` | `(h a b)` |
| `List` | `[a b]` |
| `Pair` | `[:k v]` |
| `Block` | `{c1 c2}`, with every child printed normally, so a one-call block is `{(+ 43 32)}` |
| `Arrow` | `(-> (lhs..) body)` |
| `Splat` | `(& x)` |
| `Neg` | `(#neg x)` |
| `Fallback` | `(#? a b)` |
| `Interp` | `(#interp "a " n)` |
| `IfChain` | `(#if-chain ..)` |
| `Import` | `(#import "path" as p open)` |
| `Error` | `(#error)` |

Atoms print in source syntax, and a `Builtin` prints as its bare name.
Heads starting with `#` cannot come from source, so the printed form is
unambiguous. The spec's `# ~` annotations write a one-call block as
either `(h a)` or `{h a}`. The fixture manifest therefore stores the
canonical expected string next to the verbatim annotation.

#### 6.5.5 Expander (TASK-003)

**API and files.** The signature is the plan's:
`expand(&Node, &mut ExpandCx) -> Result<Node, Diagnostic>`. It works on
one top-level form, and the first diagnostic ends expansion of that
form. `ExpandCx::new(first_free: NodeId)` gives synthesized nodes fresh
ids. Every synthesized node carries the span of the sugar node it
replaces. A form that contains an `Error` node returns
`read-error-present`, which callers do not show, because the reader has
already reported the error. Files: `expander.rs` (traversal),
`sugar.rs` (rewrites), `kernel.rs` (shape validation and `is_kernel`).
Recursion is capped at `MAX_DEPTH = 512` levels of the input tree
(`expander.rs`). The reader caps bracket nesting at 128, but pipes and
`?` folds nest calls without brackets, so past the cap the form reports
`nesting-too-deep` instead of overflowing the stack.

**Positions.**
- Expression positions are expanded. They are call heads and
  arguments, list items, pair values, splat operands, block
  statements, arrow bodies, and a match guard.
- Pattern and header positions are never expanded. They are arrow lhs
  items, `let`/`var`/`upd`/`for` targets, `fn` header items, and `enum`
  bodies. A `Neg`, `Fallback`, `Interp` or `IfChain` found there is
  `sugar-in-pattern`.
- In a `match` clause lhs, a top-level `if` splits the pattern from its
  guard. The guard must be exactly one item, and it is expanded.

**Hygiene.** Sugar produces calls to prelude natives, and it refers to
them through `Atom::Builtin` heads that resolve only in the prelude.
So a user parameter named `map` or `neg` cannot capture a desugared
`for` or `-x`. The kernel words are
`match fn let var upd enum if elif else for import`, plus `nil`,
`true` and `false`. They are reserved: binding one as a name, parameter,
fn name or target is `reserved-word`. `struct`, `inst` and the rest stay
ordinary names for the expander. Q3 is now Decided (Tidal's `struct` is
`grid`); `struct`, `inst` and `look` are definition heads that the
checker and compiler recognize (7.1.4), so the expander stays
unchanged. `while`, `loop`, `break`,
`when`, `unless` and `each` get no special treatment; they are unknown
names that the checker (TASK-004) reports.

**Desugarings** (canonical output). `E` defaults to `nil` when there is
no else. `FN` stands for the atoms `false | nil`.

| Input | Output |
|-------|--------|
| `(if C T E)`, or `(if C T)` | `(match C {(-> (FN) E) (-> (_) T)})` |
| binding `if`, pattern `P` = one identifier or `_` | `(match S {(-> (FN) E) (-> (P) T)})` |
| binding `if`, any other pattern | `(match S {(-> (P..) T) (-> (_) E)})` |
| `IfChain` | nested as above: each `elif` or `else` link is the `E` of the level before it |
| `(for P S B)` | `(match (map S (-> (P) B)) {(-> (_) nil)})`: the value is discarded and nil is returned |
| `(#? X D)` | `(or X D)` |
| `(#neg X)` | `(neg X)` |
| `(#interp p..)` | `(concat p..)`; empty text pieces are dropped, and the TASK-005 `concat` formats each part per 6.5.3 |

The two binding-if rows reproduce the two `# ==` annotations of
lang-reference section 3 exactly: the `g` case and the `ok v` case.
One edge is known. When `P` is a bare field-less variant name, the
first row's form sends a truthy value that does not match `P` to a
match failure instead of the else branch. The expander cannot tell a
variant from a binding name; only the checker knows **(U5, answered:
the expander keeps this syntactic desugaring; the checker, TASK-004,
must diagnose a bare field-less variant used as the binding pattern of
`if`)**. TASK-001..003 do not implement that diagnostic.
Guards in a binding `if` are `if-guard`. A wrong `if` arity is
`malformed-if`. A stray `elif` or `else` is `else-without-if`. A `for`
without exactly a pattern, a source and a body is `malformed-for`.

**Pairs and splats.** A call-site pair stays a `Pair` argument, which is
the 2-list value `[:k v]`. A `& x` argument stays a `Splat`, and the
compiler resolves both into keyword slots (8.1). The expander does not
reorder arguments. `put` treats pairs as plain elements and header forms
such as `inst` interleave `=`, so no position rule is imposed. A
`Splat` anywhere other than a call argument or a list item is
`misplaced-splat`.

**Kernel validation.** After expansion, a form may contain only the kinds
`Atom`, `Call`, `List`, `Block`, `Pair`, `Arrow` and `Splat`, plus
`Import` at the top level. A `Builtin` may appear only as a call head.
Each kernel head has a fixed shape:

| Head | Shape | Error code |
|------|-------|------------|
| `match` | `(match SUBJ {Arrow+})`, every clause lhs non-empty | `malformed-match` |
| `fn` | `(fn NAME HEADER* Block)`, `NAME` a `Sym` | `malformed-fn` |
| `let`, `var` | `(let TARGET EXPR)` | `malformed-binding` |
| `upd` | `(upd NAME EXPR)` | `malformed-binding` |
| `enum` | `(enum NAME Block)`, each line a `Sym` or a `Sym`-headed call | `malformed-enum` |

A `fn` body is block form only **(U4, answered)**. `name: x` in a `fn`
header is always a typed parameter, so an inline body such as
`fn kick-sound: :bd-haus` reads as a pair and the expander reports it
as `malformed-fn`. lang-reference section 4 now writes that example in
block form, so its block must read and expand clean.
The test helper `kernel::is_kernel(&Node) -> bool` checks all of
the above. The tests apply it to every expanded fixture form.

#### 6.5.6 Spec fixture manifest

`tests/fixtures/spec/manifest.toml` is written in a TOML subset, so a
real TOML parser could read it later. The subset is:
`[[block]]`/`[[case]]` headers, basic and `'''` literal strings, integers,
string arrays, and comments. A test-only parser in
`tests/support/toml_subset.rs` reads it, so no dependency is needed.
The runner is `tests/spec_fixtures.rs`. It reads the spec documents from
`CARGO_MANIFEST_DIR`.

- A `[[block]]` entry has these fields:
  - `doc` and `ordinal`: the 1-based index of the `vactr` fence in the
    document. The runner extracts the block text itself, so the source
    is never copied.
  - `section`.
  - `reader = "clean" | "diagnostics"` with
    `reader_diags = ["code@line"]`, where the line is counted within the
    block.
  - `expand = "clean" | "diagnostics"` with `expand_diags`.
  - `eval = "unclassified"`, which later tasks refine to `positive`,
    `diagnostic`, `illustrative-excluded` or `authority-question`.
  - `note`.

  Every `vactr` block in `lang-reference.md` and in `design-music.md`
  has an entry. The runner compares the diagnostic multisets exactly.
- A `[[case]]` entry has these fields:
  - `id`, `doc`, `source` (a literal string), and
    `file = "file" | "console"`.
  - `verbatim = true | false`. When true, the source must appear
    verbatim in the document, which catches drift.
  - `read` and/or `expand`: the canonical printed forms, joined by
    newlines.
  - `diags`: the expected codes.
  - `annotation`: the spec text.
  - Optional: `class = "authority-question"` and `question`.

The cases cover:
- Every `# ~` and `# ==` annotation in sections 1-3 of both documents.
  This is the acceptance bar; blocks outside those sections have
  block-level entries only.
- The negative examples `( )`, `_tmp`, `foo-` and `_1` in a file, each
  followed by a line that must still read.
- Pipe continuation.
- Byte-accurate span goldens with tabs, CRLF and UTF-8 in strings and
  comments.
- `#@` trivia.
- Imports: `import`, `as`, `open`, the fresh-document `pads.warm`, the
  `as pd` variant, and an unbound qualifier.

Two authority questions ship as fixtures with
`class = "authority-question"`. The runner asserts their read and
expand results, and their evaluation stays pending:
- `fn f a b:` / `* a 12` against `f 1 2 # => 24` (lang-reference
  section 1).
- `let base` followed by `upd base` (section 4).

The answered U4 and U5 behaviors are ordinary decided cases, with no
`class`:
- The inline `fn kick-sound: :bd-haus` gives `malformed-fn`. The spec
  no longer contains this text, so the case has `verbatim = false`.
- The bare-variant binding `if` (`if x none -> 1`) expands to
  `(match x {(-> (false | nil) nil) (-> (none) 1)})`. Its `note`
  records the TASK-004 diagnostic obligation.

A no-panic test reads every line-boundary prefix of every block.

#### 6.5.7 Verification and rollout constraints

- Lint uses the stricter `cargo clippy --all-targets -- -D warnings`,
  which also covers tests.
- The `wasm32-unknown-unknown` target is installed **(U1, answered)**,
  so the wasm32 criterion of TASK-001 is run and must pass. Two builds
  count: `cargo build --target wasm32-unknown-unknown` (default
  features) and the same with `--no-default-features --features
  host-wasm`. Both work because every feature is empty at this stage.
  The core stays wasm-safe by construction: no `std::thread`,
  `std::fs`, `std::time`, `std::net` or `std::process` in any core
  module. A failing wasm32 build fails TASK-001; it is not recorded as
  blocked.
- Rust 1.83, edition 2021: no let-chains.
- No `.rs` file may reach 1000 lines (section 4 note).
- **Verification evidence** (added 2026-09-25, session 169, after the
  FE-VALUE gate rejected a clippy log that a rerun had overwritten).
  This rule applies to every cargo check in the four FE plans, and it
  overrides the fixed log names in the plans' verification tables:
  - Every run writes a new file,
    `target/fe-logs/<plan>-<check>-s<session>-<n>.log`. `<plan>` is
    `value`, `reader`, `expand` or `final`. `<check>` is the plan's name
    for the check (`build`, `clippy`, `nextest`, `wasm32`,
    `wasm32-hostwasm`). `<n>` counts up from 1 within the session. No
    run may overwrite an existing log, including logs that an earlier
    session cited.
  - Each log ends with its exit status. Run
    `(set -o pipefail; CMD 2>&1 | tee LOG); echo "exit=$?" >> LOG`,
    which works in both bash and zsh. The plans' `${PIPESTATUS[0]}` is
    bash-only. A quiet build that succeeds writes nothing else, so the
    `exit=` line is what separates it from a run that never finished.
  - The progress log cites, for each check, the path of the log that
    counts and its `exit=` value. For nextest it also cites the run and
    passed counts, and the run count must be non-zero. A log that is
    missing, has no `exit=` line, or was cut short fails the check.
  - Reruns stay on disk. The evidence is the last run of each check
    after the final code change.
- Rollback is `git revert` of the task commits. There is nothing to
  migrate.

#### 6.5.8 Path and url literals (front-end amendment, issue #2)

Implements lang-reference section 3 "path and url literals" (Decided
2026-09-25). It is the FRONTEND wave of 7.1.7 and lands before the
checker types these literals.

**Lexing** (`src/reader/pathlit.rs`, called from `lexer.rs`, so
`lexer.rs` stays under 800 lines). A path or url token can start only
where a keyword can: at the start of a line, or right after whitespace,
`{` or `[`. It is tried before the number, operator and name rules.
- **Path.** The token starts with one of the prefixes `./`, `../`, `~/`
  or `/` directly followed by a path character, and runs while the next
  character is a path character `[A-Za-z0-9._~-]` or `/`. It ends at any
  other character; whitespace, `}`, `]`, `#` and `:` are the usual
  ends. The whole token must match
  `PREFIX SEG ("/" SEG)*`, where `SEG` is one or more path characters.
  An empty segment (`.//x`), a trailing `/` (`./dir/`), or a bare prefix
  (`./`, `~/`) is `bad-path`. `.` and `..` segments are allowed and are
  kept as written; normalization is the host's job.
- A `/` followed by whitespace, or at the end of the line, stays the
  division operator, so `/ a b` is unchanged. A `/` inside a number is
  unchanged too: `1/4` is a ratio, since it does not start at a token
  boundary after a digit. `..` directly followed by `/` at a token start
  is a path, never a range: `a..b` and `0..8` start with a digit or
  name.
- **Url.** The token is `SCHEME "://" REST`. `SCHEME` is
  `[a-z][a-z0-9+.-]*`. `REST` is one or more characters from printable
  ASCII other than whitespace, `"`, `#`, `{`, `}`, `[`, `]`, `\`, `<`,
  `>`, `|`, `^` and backtick. So a url ends at whitespace, at a closing
  bracket, or at a comment. Url fragments (`#frag`) are not part of a
  url in v1: a `#` ends the url and starts a comment. An empty `REST`
  is `bad-url`. Without the `://` the token is lexed as before (a name
  followed by a colon), so no current program changes meaning.
- A path or url is not a pair-key token: in `./x: y` the colon follows
  the colon-class rules of 6.5.4 as if it came after a string that is
  not a key, so it is a block opener at the end of the line and
  otherwise `misplaced-colon`.
- A path token that contains a character outside the path set before
  its end (for example `./a!b`) ends at that character, which is then
  lexed on its own (`!` is `stray-char`). There is no other path
  diagnostic.

**Nodes and printing.** `Atom` gains `Path(Rc<str>)` and `Url(Rc<str>)`,
holding the literal text as written. The canonical printer (6.5.4)
prints both as written, so `./soundpack/bd/1.wav` round-trips. The
expander passes both through unchanged; they are not names, so they are
never `reserved-word` and never captured.

**Values** (`src/value/value.rs`, TASK-001 files).
- `Value::Path(Rc<PathVal>)` with `PathVal { text: Rc<str>, file:
  Option<FileId> }`. `file` is the `FileId` of the literal's span for a
  relative path (`./`, `../`), and `None` for `/` and `~/` paths. This
  is how "relative to the containing file" is carried: the host maps a
  `FileId` to a location when it performs I/O (TASK-007/008). The
  console and the session buffer have ids too (`FileId::CONSOLE` and
  the buffer's id), and the host resolves those against the project
  root. No stage in this issue resolves, normalizes or expands a path.
- `Value::Url(Rc<str>)`, holding the text as written.
- `Value::Sound(Rc<Sound>)` with `enum Sound { Builtin(KwId),
  Sample(PathVal), MidiOut(u8) }`: a builtin host sound named by
  keyword, a sample file named by path (no I/O in this issue), or a
  MIDI-out channel (`{midi 1}`, design-music "sources and
  destinations"). TASK-008 adds the `inst` template variant. A sample
  BANK is an ordinary list of sounds (7.1.4).
- Equality (`eq.rs`): `Path` equals a `Path` with the same text and the
  same `file`; `Url` compares by text; `Sound` compares structurally.
  None of them is a dict key (`key.rs` is unchanged): using one as a key
  is `Failure(Type)`, as for any non-key value.
- Printing (`print.rs`): a path and a url print as their text. A sound
  prints as `(sound :bd)`, `(sound ./bd/1.wav)` or `(sound midi 1)`.
- Accessors treat all three as scalars: `len` and `index` are
  `Failure(Type)`, and they are truthy.

**Tests.** Lexer tests cover the four path prefixes, a url, each
terminator (whitespace, `}`, `]`, `#`, `:`), `bad-path` for the three
malformed shapes, `bad-url`, and the unchanged readings of `/ a b`,
`1/4`, `-1/4`, `0..8`, `a..b`, `amp: 0.5`, `~` alone (`stray-char`) and
a url-looking `name:x` (`misplaced-colon`). The three spec blocks now
marked PENDING in the manifest drop the PENDING text:
lang-reference block 3 and design-music block 2 read clean, and
lang-reference block 5 keeps only the `stray-char` of the `...`
placeholder in `fn pluck ...` (`...` is not a path: its third character
is not `/`).

## 7. Static Checker and Inference

HM-lite with let-polymorphism (decided): unification over

```
int int64 float float64 ratio bool string keyword nil  ?T  [T]  [K: V]
fn T… -> V   pattern T   signal   any   path   url   sound
+ named struct/enum types
```

plus numeric-literal kind variables (a literal adapts to context;
default int/float), keyword literal typing against the enum or sound
kit expected at that position (`manifest.rs` holds the host's builtin
sound, synth, and control sets, from which the prelude's
`default-sound-kit` is built, so `:bd-haus` completes and
`:not-a-sample` is a diagnostic; 7.1.4 gives the `sound-kit` rule),
`?T` optional flow (`?` supplies the default; using a
`?T` as `T` in arithmetic/calls is a diagnostic; accessors nil-pun),
and thunk parameter typing (section 5.5). `any` must be narrowed by a
match/if pattern before use.

Checker output is a `TypedInfo` side table (`NodeId -> Ty`) for hover,
plus a `Vec<Diagnostic>`:

```rust
pub struct Diagnostic { span: Span, severity: Severity, code: DiagCode, message: String,
                        origin: Option<RunOrigin> }   // RunOrigin: slot + beat, runtime only
```

Live mode: every type error is a diagnostic; the code still compiles
and runs on the dynamically checked VM (a fault at runtime is then a
failure with origin). Frozen mode (later) turns the same list into
compile errors. Static diagnostics also cover: literal `/ x 0`,
undefined names, unknown sample/synth keywords, `match` missing an enum
variant, annotation mismatches, duplicate dict-literal keys, same-scope
rebinding and shadowing (5.6 scope model), effectful calls inside pattern arguments (purity warning,
section 10.4), and unbounded-source iteration where provable.

The checker is one crate module consumed identically by the compiler,
the LSP (section 14.3), and the session's eval path — "LSP sharing the
checker" is literal reuse, not a port.

### 7.1 Middle-end implementation decisions (TASK-004..006, 2026-09-25)

This section pins the choices that sections 5.5-5.6, 7-10, 11.1 and
11.7 leave open for issue #2 (checker, namespace/compiler/VM, pattern
engine). It follows the shape of 6.5 and changes no Decided behavior.
It also carries the 2026-09-25 amendments bound to issue #2: the
parent-scope model (5.6), path and url literals (6.5.8), `load` and
`sample`, the sound-kit prelude bindings, SOUND FIRST with the
first-structure rule (10.1), and `midi-notes` as a step after `s`
(11.7). TASK-007..010 stay out of scope: the slot table, the
two-horizon scheduler, layer-2 dedup, capability hosts (beyond the
`NoopHost` source-loader stub of 7.1.3), host file and sample I/O,
DSP and package loading.

#### 7.1.1 Pipeline and gating

Every top-level form goes read -> expand -> check -> compile -> run.

- A form with a reader or expander error is not checked, compiled or
  run. Its diagnostics are the front end's (6.5).
- Check diagnostics never gate compile or run (Live mode, section 7).
  `check()` always returns a `CheckResult`; a node it cannot type gets
  `Any` with a diagnostic, and it never returns early for the form.
- `compile` fails a form only on `nesting-too-deep` (7.1.5). The form
  is then not run and the diagnostic is reported.
- `compile` always calls `infer_masks` on its own and does not depend on
  the diagnostics pass (5.5). With diagnostics off, the masks are
  identical.
- Frozen mode is not implemented.

#### 7.1.2 Files and the 800-line budget

No `.rs` file may reach 800 lines. The hard limit is 1000 (section 4).
A wave checks this before it completes: the largest `.rs` line count
is recorded in its evidence. Tests go in `tests/` submodules, as in 6.5.

| Module | Files (owner wave, 7.1.7) |
|--------|---------------------------|
| `reader/` | `pathlit.rs`, edits to `lexer.rs`, `node.rs`, `sexpr.rs` (FRONTEND) |
| `value/` | edits to `value.rs`, `eq.rs`, `print.rs`, `access.rs` for `Path`, `Url`, `Sound` (FRONTEND) |
| `types/` | `ty.rs`, `masks.rs`, `natives.rs`, and every 7.1.6 code in `diag.rs` (MASKS); `infer.rs`, `unify.rs`, `check.rs`, `scope.rs` (scope chain and the 5.6 diagnostics), `manifest.rs`, `deps.rs` (CHECK) |
| `compile/` | `compiler.rs`, `matchc.rs` (match to pattern ops), `proto.rs`, `sites.rs` (tweak sites and tiers) (VM) |
| `vm/` | `ops.rs`, `frame.rs`, `vm.rs`, `call.rs` (boundary forcing and `Forward` chase), `natives/{num,list,dict,value,console,effects}.rs` (VM); `query_vm.rs`, `natives/{pattern,signal,tex,music,sound}.rs` (INTEGRATE; `sound.rs` holds `s`, `sample`, `midi`) |
| `ns/` | `namespace.rs` (prelude, session and child scopes), `tweak.rs`, `pkg.rs` (`PkgNs`, `ImportBinding`), `stage.rs` (VM); `depgraph.rs`, `journal.rs`, `evaluator.rs`, `load.rs` (`SourceLoader`, `NoopHost`, the `load` native) (REACTIVE) |
| `pattern/` | `pat.rs`, `step.rs`, `query.rs`, `occ.rs`, `rng.rs`, `signal.rs`, `eval.rs`, `combinators/{time,structure,random,region,music,control,input}.rs` (PATTERN) |
| `clock/` | `tempo.rs`, `clock.rs` (`Tempo`, `Clock`, `ClockSource`, `:midi` anchor math) (PATTERN) |
| `tex/` | `texnode.rs`, `shader.rs` (snippets and `compile_tex`), `uniforms.rs` (PATTERN) |

The clock lives in `src/clock/`, as in section 4 and the core plan, not
in `src/sched/`. `sched/` stays TASK-007's.

#### 7.1.3 Contracts between waves

- **Masks.** `MaskEntry`, `ForcingMask`, `CalleeRef` and `infer_masks`
  are defined once, in `types/masks.rs`. The compiler and the VM import
  them and never redefine them. The masks-based `mixed-forcing` check
  over fan-out link sets lives there as well.
- **Native signature table** (`types/natives.rs`). There is one entry
  per prelude name in scope for this issue, indexed by `NativeId`. An
  entry holds the name, the arity and keyword parameters, the type
  scheme, the per-parameter forcing mask, and the overload group
  (7.1.4). The checker reads the types, `infer_masks` reads the masks,
  and the VM registers each implementation against its entry. A test
  asserts that every registered native has exactly one entry with the
  same arity and mask, and that every entry has an implementation. In
  scope: the lang-reference section 5 core prelude, the design-music
  pattern, control and signal vocabulary, `d1`..`d9`/`slot`, `once`,
  `at`, `hush`, `stop`, `use-bpm`, `use-cycle`, `midi-notes`, `cc`,
  `s`/`sound` (keyword parameter `kit`), `sample`, `midi`, `load`, the
  prelude values
  `default-sound-kit` and `sound-kit`, and the design-visual
  vocabulary. Out of scope and absent from the table,
  so the checker reports them as `undefined-name`: the synthesis, effect,
  bus and granular vocabulary (TASK-008) and package loading (TASK-009).
- **Codes.** The MASKS wave adds every `DiagCode` and `FailCode` in
  7.1.6 in one edit, including the FRONTEND reader codes, so FRONTEND,
  CHECK, VM and PATTERN start after it. No other
  wave edits `types/diag.rs` or `vm/fail.rs` while the parallel waves
  run. If a code turns out to be missing, a wave uses the closest listed
  code, records a finding, and the INTEGRATE wave adds the code.
- **Query VM handle** (`pattern/eval.rs`, the "VM handle" of 10.3). The
  PATTERN wave defines the trait `QueryVm`:
  - `call(f, args) -> Result<Value, Failure>`, for `PParam::Fn`,
    `VParam::Fn` and transform closures;
  - `deref(&VarSlotRef) -> Result<Value, Failure>`, for `Late`;
  - `take_output() -> Vec<(Origin, Rc<str>)>`, for captured `print`;
  - `sound_kit() -> Result<Value, Failure>`, the current value of the
    SESSION-level `sound-kit`: the session binding when one exists,
    otherwise the prelude's. A `PatNode::Sound` calls it per query only
    when its `kit` is `None`, that is, when the `s` call had no `kit:`
    (7.1.4, 10.1).

  The implementation enters Query effect mode with a scope guard on
  every call (10.4). The PATTERN wave tests with a stub. The INTEGRATE
  wave implements the trait for `Vm` in `vm/query_vm.rs`. `pattern/`
  and `tex/` treat `VarSlotRef` and `Closure` as opaque, and never edit
  `value/`, `ns/` or `vm/`.
- **Input cells.** `Sig::Cc` and `Sig::Analyzer` read `f32` values from
  an `InputCells` table that is passed into query and frame evaluation,
  and so do the host signals `Sig::Host` (`fft`, `amp`). It is keyed by
  (channel, controller), by `AnalyzerId` and by `HostSig`. Only tests
  write it in this issue; TASK-007/008 hosts write it later.
- **Staged effects** (`ns/stage.rs`, VM wave). Every host-visible effect
  is a `StagedEffect`: a slot bind (pattern or texture), a revocation,
  a control-cell update, a tweak refresh, the `bindings` batch, a tempo
  change, a one-shot schedule (`once`/`at`), or a console line. A form
  collects its effects in a buffer. They are released to an
  `EffectSink` only when the whole form succeeds, and dropped when it
  fails (the bind-then-fail case). The only sink in this issue is the
  test `RecordingSink`; TASK-007 adds the slot-table sink. The REACTIVE
  wave builds pass-level staging and the pass journal on top of this
  buffer: a pass releases nothing until validation.
- **Top-level driver** (`ns/evaluator.rs`, REACTIVE wave). `Evaluator`
  owns the `Namespace`, `DepGraph`, `Vm`, staging and `FormGen`
  counter. `eval_form` runs one expanded form and returns its outcome
  with diagnostics. A top-level `upd` or a redefinition triggers the
  reactive pass of 5.6, coalesced latest-wins per tick. TASK-009's
  `Session` wraps `Evaluator` and does not re-implement it.
- **Source loading** (`ns/load.rs`, REACTIVE wave). The one host
  capability in this issue is `trait SourceLoader { fn read(&mut self,
  path: &PathVal) -> Result<(FileId, Rc<str>), Failure>; }`. The loader
  resolves the path (6.5.8) and assigns the `FileId` of the file it
  returns. `Evaluator` holds a `Box<dyn SourceLoader>`; the default is
  `NoopHost`, whose `read` is always `Failure(host-unavailable)`.
  Tests use an in-memory map loader. TASK-007 moves `NoopHost` to
  `host/noop.rs` and adds the real loaders; it keeps this trait.
  `load p` runs the returned text through read -> expand -> check ->
  compile -> run in a fresh scope whose parent is the prelude, and
  returns the value of its last top-level form (`nil` for an empty
  file). The loaded file's bindings are not visible to the caller. Any
  reader or expander error, or a run failure, in the file is
  `Failure(load-failed)` for the `load` call, carrying the first cause;
  its check diagnostics are reported with their own spans and do not
  fail the load (7.1.1). A nested `load` counts as one Rust-level
  re-entry (7.1.5), so a load cycle ends in `depth-exceeded`. `load`
  is an effect: inside a query it is `effect-in-query`. `sample` does
  no I/O here: it returns `Sound::Sample(path)`, and reading the file
  is TASK-008's `SampleLoader`.

#### 7.1.4 Checker and runtime rules pinned here

- **Scopes and shadowing (20 Q1, Decided 2026-09-25).** The checker
  follows the 5.6 scope model. A binding is a `let`/`var`/`fn` name, a
  parameter, or a pattern binding in a `match` clause or `let [..]`
  target.
  - The same name bound twice in one scope is `rebinding` (error), on
    the second binding. Checking a document or spec block checks all
    its top-level forms as one session scope, so
    `let a 12` then `let a 13` is `rebinding`. `Evaluator::eval_form`
    of one form never reports it: that is Live redefinition.
  - A binding whose name resolves in the prelude is `shadows-prelude`
    (hint), for example `let sound-kit ...` or `let scale 2`.
  - A binding whose name resolves to a user binding of an enclosing
    scope is `shadowing` (warning).
  - `upd` of a name that is not a `var` (a `let`, a `fn`, a parameter,
    a pattern binding or any prelude name) is `upd-immutable` (error).
    The VM fails the same `upd` with `Failure(upd-immutable)`.
  - `inst` header parameters are control names, not bindings (M3), so
    `amp: float = 0.5` in an `inst` header never gets a scope
    diagnostic.
  - Hints are for the LSP. Fixture multisets (7.1.7) list errors and
    warnings only; unit tests assert the hints.
- **Definition heads.** The checker and compiler recognize top-level
  `struct NAME:` blocks as definitions (lang-reference section 2).
  `inst` and `look` are recognized as definition heads that own a slot.
  Their bodies are type-checked for the no-abort property only, and the
  compiler classifies `inst` header default literals as `Direct`-tier
  tweak sites from the form's structure (section 13). Building the
  instrument (`InstDef`, `Ctl::Cell`) is TASK-008's.
- **Bare-variant binding (U5).** A `match` with exactly two clauses,
  where the first clause's left side is the alternative `false | nil`
  and the second clause's left side is one bare name that resolves to
  a field-less enum variant, is `bare-variant-binding` (error). This is
  the shape `if S P -> T` desugars to, so a truthy value that is not
  the variant fails instead of taking the else branch. The hand-written
  identical `match` gets the same diagnostic, because it is the same
  trap. The message suggests `match S` with the variant and `_`, or
  `if {= S none}`.
- **Chord qualities (Decided 2026-09-25).** Qualities are letter-first
  keywords. The v1 set and intervals are `:maj7` (0 4 7 11), `:m7`
  (0 3 7 10), `:dom7` (0 4 7 10) and `:sus4` (0 5 7). A literal quality
  outside the set is `unknown-keyword` (the keyword-set mechanism of
  section 7). A dynamic one is an event-local query fault. `:7` never
  reaches the checker: it is `misplaced-colon` in the reader. Adding a
  quality means adding a row to the table.
- **`grid` (20 Q3, Decided).** `grid` builds `PatNode::Grid`. `struct`
  is never a pattern function.
- **`gain` and `amp` (20 Q4, Decided).** `gain` is a pattern control
  native. The `gain` -> `amp` mapping is a control-table row owned by
  TASK-007/008.
- **Sounds and the sound kit (Decided 2026-09-25, design-music.md
  "sound kits").** The prelude binds `default-sound-kit` and
  `sound-kit`, both of type `[keyword: sound]` and initially the same
  dict: one `Sound::Builtin(k)` per keyword `k` of the host manifest's
  builtin sound set (`manifest.rs`). `sample : fn path -> sound` and
  `midi : fn int -> sound` (channel 1..16; a literal outside the range
  is `type-mismatch`, a dynamic one is `Failure(type)`).
  - A list of sounds is a sample bank. The unifier accepts `[sound]`
    where `sound` is expected, and only there, so a pack dict such as
    `[bd: [sample ./bd/1.wav sample ./bd/2.wav] sd: sample ./sd.wav]`
    types as `[keyword: sound]`. When the resolved sound is a bank,
    `n i` picks its i-th sound at event time, and an index outside the
    bank is an event-local `Failure(slice-index)`. For any other sound
    `n` stays an ordinary control (the host's sample index, TASK-008).
  - `s` (alias `sound`) takes exactly one positional argument of type
    `pattern sound`, and one optional named argument `kit:` of type
    `[keyword: sound]` (Decided 2026-09-25, design-music.md "sound
    kits"). `s [:bd :sd] kit: tr909` reads as
    `(s [:bd :sd] [:kit tr909])`; the `kit:` pair is not a positional
    argument. Any other named argument is `type-mismatch`.
  - A keyword in a `sound` position (directly, in a step list, or as an
    argument of a step constructor such as `alt` or `choose`) NAMES a
    sound in the kit and has type `sound`. Any other argument is a sound
    VALUE and is used as is, with no kit lookup: `let kick sample
    ./kick.wav` then `s kick`, or `s {midi 1}`.
  - Static check: a keyword literal is checked against the key set of
    `default-sound-kit` plus the names of the `inst` definitions in the
    checked document (TASK-008 registers them), and a miss is
    `unknown-keyword`. The check runs only when the call has no `kit:`
    and `sound-kit` resolves to the prelude at the call site. When
    `kit:` is given, or the session binds `sound-kit`, the checker
    cannot know the keys (they may come from `load`), so it skips the
    check.
  - At run time `s` stores the keywords and resolves them per query
    (10.1). The kit is the `kit:` value when the call has one (a
    `PParam`, so a `var` kit is read per query). Otherwise it is
    `QueryVm::sound_kit()` (7.1.3), so a new session `sound-kit`
    binding is heard from the next event on, as the spec requires. A
    key missing from the kit in use is an event-local
    `Failure(unknown-sound)`. A local binding named `sound-kit` inside a
    `fn` does not affect `s`; only `kit:` and the session-level binding
    do.
  - `sound-kit` in the prelude is read-only like every prelude name;
    overriding it is a session `let`, which is `shadows-prelude`
    (hint). `put` merges kits (6.5.3 dict `put`: later pairs win).
- **SOUND FIRST (Decided 2026-09-25, design-music.md section 1).** A
  call to `s`/`sound` with two or more positional (non-pair) arguments
  is
  `sound-not-first` (error). That is how `n [0 3] > s :bd` and
  `note [:c] > s :x` read: the pipe puts the pattern in as the first
  argument (6.5.4), giving `(s (n [0 3]) :bd)`. The message says that
  `s` starts the chain and suggests `s :bd > n [0 3]`. `s` given a
  single non-sound pattern (`s {n [0 3]}`) is an ordinary
  `type-mismatch`. The runtime rule for structure is 10.1.
- **`load` and `sample` typing.** `load : fn path -> 'a`, where `'a` is
  a fresh type variable at each call site. The file's type is not known
  until the host reads it, so the result unifies with its use (for
  example `put default-sound-kit my-pack` makes it `[keyword: sound]`)
  and the VM checks it dynamically. A `url` argument to `load` or
  `sample` is `type-mismatch` in this issue; loading urls is TASK-009's
  package proxy. A path literal has type `path` and a url literal has
  type `url`; neither converts implicitly to or from `string`.
- **Subject overloading (20 Q2, M1 answered 2026-09-25: follow the
  recommendation).** TASK-006 implements a fixed set only:
  - `scale`: a pattern subject means scale notes; a texture subject
    means the visual transform.
  - `shape`: a number subject means the visual source; a pattern
    subject means the `shape` control.

  The checker resolves the overload when the first argument's type is
  known. Otherwise the VM's native dispatch switches on the runtime tag,
  and a tag outside the group is a `type` failure. User code cannot
  declare overloads.
- **`range` argument order (M2 answered 2026-09-25).** `range` is
  subject first (`range sine 200 2000`), and design-music section 3
  already reads `lpf {range sine 200 2000}`. No fixture pins a
  `type-mismatch` for it.

#### 7.1.5 Resource bounds (no panic on untrusted input)

- The checker and the compiler each count recursion depth over the
  expanded tree. Past 1024 levels they report `nesting-too-deep`: the
  checker types the subtree as `Any`, and the compiler skips the form.
  An `if`/`elif` chain expands into nested `match` forms, so the
  expanded depth can exceed the expander's input cap of 512.
- VM defaults (configurable on `Vm`): `depth_limit` = 512 frames, and at
  most 64 nested Rust-level re-entries. A re-entry is a native calling
  back into a closure, or a `QueryVm` call. Exceeding either is
  `depth-exceeded`. Fuel is 1,000,000 per top-level evaluation and per
  `QueryVm` call. Exhausting it is `fuel-exhausted`; this is the
  "unbounded source" failure of `for x 0..:`.
- All `Ratio64` arithmetic in query, clock and region math goes through
  the checked operations of 5.3. Overflow is a `Failure`, event-local
  or subtree-local per 10.3, and is never a panic.
- There is no `std::thread`, `std::time`, `std::fs`, `std::net` or
  `std::process` in `types/`, `ns/`, `compile/`, `vm/`, `pattern/`,
  `clock/` or `tex/`. The `:midi` clock math takes pulse timestamps as
  arguments.

#### 7.1.6 Diagnostic and failure codes (closed list for this issue)

New `DiagCode`s (kebab-case, as in 6.5). Severity is error unless it
says (w) for warning or (h) for hint:
- Reader (FRONTEND, 6.5.8): `bad-path`, `bad-url`.
- Types and names: `type-mismatch`, `annotation-mismatch`,
  `optional-as-value`, `any-not-narrowed`, `undefined-name`,
  `rebinding`, `shadowing` (w), `shadows-prelude` (h),
  `upd-immutable`, `literal-division-by-zero`, `unknown-keyword`,
  `missing-variant`, `bare-variant-binding`, `sound-not-first`.
- Hygiene: `duplicate-key` (w), `import-collision` (w),
  `beyond-capability`, `effect-in-pattern` (w), `unbounded-source` (w).
- Forcing: `mixed-forcing` (w), `latent-forcing` (w).
- Patterns and clock: `bad-slice-points`, `input-lane-operator`,
  `clock-source-unavailable`.
- Reactive: `dependency-cycle`.

New `FailCode`s: `no-match`, `fuel-exhausted`, `depth-exceeded`,
`effect-in-query`, `effect-in-rebuild`, `undefined-name`,
`not-callable`, `arity`, `blocked` (a form reported `blocked-on: X`,
5.6), `slice-index`, `bad-slice-points` (dynamic points),
`no-whole` (a region operator got an event with `whole = None`),
`upd-immutable`, `unknown-sound` (a key missing from the current
`sound-kit`, event-local), `host-unavailable` (`NoopHost`), and
`load-failed` (7.1.3).

#### 7.1.7 Waves, fixtures and verification

**Waves.** A manifest `dependsOn` appears only where source forces
sequencing. Write sets are disjoint; the file owners are in 7.1.2.

| Wave | Content | Depends on |
|------|---------|------------|
| MASKS | `types/ty.rs` (including `path`, `url`, `sound`), `masks.rs`, `natives.rs`; the code lists of 7.1.6 | — |
| FRONTEND | 6.5.8: path/url lexing, `Atom::Path`/`Url`, `Value::Path`/`Url`/`Sound`, the three PENDING manifest blocks | MASKS (the reader codes) |
| CHECK | the rest of TASK-004, including the scope diagnostics, sound-kit, SOUND FIRST and `load`/`sample` typing of 7.1.4 | MASKS, FRONTEND |
| VM | TASK-005 namespace (prelude, session and child scopes), compiler, VM, core natives, tweak sites, `PkgNs`, `stage.rs` | MASKS, FRONTEND |
| REACTIVE | TASK-005 `DepGraph`, pass journal, `Evaluator`, `load.rs`, rebuild and reactive-propagation tests | VM |
| PATTERN | TASK-006 `pattern/`, `clock/`, `tex/`, including the first-structure rule and `midi-notes` after `s` (10.1, 11.7), with unit and golden tests through the stub `QueryVm` | MASKS (the 7.1.6 codes), FRONTEND (`Value::Sound`) |
| INTEGRATE | `QueryVm` for `Vm` (including `sound_kit`), domain native wrappers (`natives/sound.rs`), fixture evaluation stages, `src/lib.rs` crate doc, TASK-006 criteria that need the VM (`PParam::Late`, `PParam::Fn`, the tweaked-probability re-query, a session `sound-kit` heard at the next event, `kit:` resolution and a `var` kit read per query) | CHECK, REACTIVE, PATTERN |

Only PATTERN adds a module to `src/lib.rs` (`pub mod clock;`).
INTEGRATE edits the crate doc afterwards and mentions the expander (a
carried TODO).

**Spec fixture evaluation** (extends 6.5.6). `eval = "unclassified"`
is replaced on every block by one of these classes:
- `positive`: every form in the block checks with no error-severity
  diagnostic, compiles, and runs without failure in one isolated
  `Evaluator`.
- `diagnostic`: `check_diags` and `run_fails` pin the exact multisets
  as `code@line`.
- `authority-question`: evaluation stays pending (6.5.6).
- `illustrative-excluded`: not evaluated. The `note` says why.
- `deferred`, with `deferred_to = "TASK-00N"`: the block needs a runtime
  outside this issue (DSP instruments, buses, package loading). It is
  checked for the no-panic and no-abort property only, and it is not
  compiled or run. A block deferred ONLY because it needs host file I/O
  (`load` or `sample` of a path, TASK-008) also pins `check_diags`, so
  that `load path` and `sample path` are shown to type-check.

A block or case is checked as one document: all its forms are checked
together as one session scope (so `rebinding` is found), then run form
by form. `check_diags` lists errors and warnings, never hints.

Every `# => v` annotation in lang-reference sections 1-5 gets a
`[[case]]`. The case holds `value` (the canonical print, 6.5.3) or
`fail` (a `FailCode`), plus `check_diags`, and it is evaluated in a
fresh `Evaluator`. The annotated negatives (`+ 1 "a"`, `?T` used as
`T`, rebinding, `upd` of a `let`) are cases with their codes
(`rebinding`, `upd-immutable`). Scope, sound-kit and SOUND FIRST
negatives get non-verbatim cases: `let a 1` twice (`rebinding`), a
`fn` parameter shadowing a session name (`shadowing`),
`n [0 3] > s :bd > d1` and `note [:c] > s :x > d1`
(`sound-not-first`), `s :not-a-sound > d1` (`unknown-keyword`), and
`s :bd909 kit: [bd: {sample ./bd.wav}] > d1` (checks clean because
`kit:` skips the static check; the query fails event-locally with
`unknown-sound`).
Positive non-verbatim cases pin the sound-kit lines of design-music
section 2 one at a time: `let sound-kit put
default-sound-kit [bd: {sample ./bd/909.wav}]` then
`s :bd > n [0 3] > d1` checks clean (one `shadows-prelude` hint) and
runs. `kit:` gets two cases:
`let tr909 [bd909: {sample ./tr909/bd.wav} sd909: {sample ./tr909/sd.wav}]`
then `s [:bd909 :sd909] kit: tr909 > d1` checks clean (no
`unknown-keyword`, although neither key is in `default-sound-kit`), runs,
and its queried events carry `Sound::Sample` of the two tr909 paths;
`let kick sample ./kick.wav` then `s kick > d1` checks clean and its
events carry that sample without a kit lookup. That spec block binds `sound-kit` twice (two alternative
overrides), which is `rebinding` when read as one document; the block
is `deferred` (TASK-008, it defines an `inst`), so nothing pins that
yet (question M6 in
`design-docs/user-qa/pending-middle-end-questions.md`). Chord disposition: blocks 2
and 3 of design-music.md read clean with `[:g :dom7]`. Case
`music-chord-seven-conflict` stays as a `verbatim = false` negative
reader case (`misplaced-colon`), and `src/reader/tests/lexer.rs` keeps
its `[:g :7]` assertion.

**Verification.** The 6.5.7 evidence rule applies to every wave, with
`<plan>` being the wave's short name (`masks`, `frontend`, `check`,
`vm`, `reactive`, `pattern`, `integrate`, `final`). The checks are `build`,
`clippy` (`--all-targets -- -D warnings`), `fmt` (`--check`),
`nextest`, `test` (a plain `CARGO_TERM_QUIET=true cargo test`),
`wasm32`, `wasm32-hostwasm` and `linecount`. Rollback is `git revert` of
the single implementation commit; there is nothing to migrate.

## 8. Bytecode, Frames, and the VM

### 8.1 Compilation units

```rust
pub struct FnProto { arity: Arity, code: Vec<Op>, consts: Vec<Value>,
                     locals: u16, spans: Vec<(u32, Span)>, name: Option<SymId> }
pub enum Arity { Fixed(u8), WithKeywords { fixed: u8, keys: Box<[(KwId, Value)]> } }
```

Keyword parameters with defaults are decided header syntax; the
compiler resolves trailing pairs and `& opts` splats at the call site
into the callee's keyword slots.

### 8.2 Instruction set (sketch)

Stack machine, one operand stack, frame array. Because the kernel has
**no loop form**, bytecode has no back edges: `match` compiles to
forward tests and jumps; iteration lives in native `map`/`filter`/
`reduce`/`find`/`take` with a fuel budget. Recursion is a frame-depth
limit (a failure, no TCO — decided).

```
LoadConst k | LoadLocal i | StoreLocal i | LoadGlobal g | LoadGlobalRef g
LoadTweak t                                  // section 13
MakeList n | MakeDict n | MakeClosure p | MakeThunk p
Call n | CallKw n k | CallValue n            // list/dict/variant/fn in head position
Force | Deref                                // thunk / VarRef demand points
JumpIfNoMatch pat, off | Jump off            // match machinery (pattern ops below)
TestLit k | TestVariant v | BindField i | TestLen n | SplitRest n   // pattern ops
Fail code | Pop | Ret
```

`Force` implements call-boundary forcing per the callee's mask (5.5)
and statement-position blocks; `Deref` resolves `VarRef` at the demand
points of 5.6. In Query effect mode (10.4), ops that perform prohibited
effects fail instead of executing.

### 8.3 Frames and the interpreter loop

```rust
pub struct Frame { proto: Rc<FnProto>, ip: u32, base: u32 }
pub struct Vm { stack: Vec<Value>, frames: Vec<Frame>, fuel: u64, depth_limit: u32 }
```

`Vm::run` returns `Result<Value, Failure>`. The fuel counter decrements
per instruction and per native-iteration step; exhaustion (e.g. `for x
0..:` with no `take`) is the decided "unbounded source" failure, so a
live session cannot hang.

### 8.4 Failure and unwinding

```rust
pub struct Failure { code: FailCode, message: String, origin: Origin }
pub struct Origin { span: Option<Span>, slot: Option<KwId>, beat: Option<Ratio64> }
```

No error type in the language: a failure unwinds through Rust `Result`
to the top level or the current pattern event, is reported on the
diagnostics bus with its origin, and the session continues. `_1` in the
console is not written by a failed expression. A failing event is
dropped; its slot keeps playing. No try/catch anywhere.

## 9. Visual Chains and the Render Path

Visuals are specified by `design-visual.md` independently of the
deferred music-visual coupling; this section designs their runtime.
Nothing here couples the two languages.

### 9.1 Representation

```rust
pub struct TexNode { kind: TexKind, params: Box<[VParam]>, span: Option<Span> }
pub enum TexKind {
    Osc, Noise, Voronoi, Shape, Gradient, Solid, Text(Rc<str>), SrcOut(OutId),
    Rotate, Scale, Pixelate, Tile, TileX, TileY, Kaleid, Scroll,
    Posterize, Shift, Invert, Contrast, Brightness, Luma, Thresh,
    Color, Saturate, Hue, Colorama,
    Blend(BlendOp, Rc<TexNode>),          // add sub layer blend mult diff mask
    Modulate(ModKind, Rc<TexNode>),
    Chain(Rc<TexNode>, Rc<TexNode>),      // left-to-right pipe
}
pub enum VParam { Const(f32), Late(VarSlotRef), Sig(Rc<Sig>), Pat(Rc<Pat>), Fn(Value) }
```

Hydra operator chains build this tree on the evaluator thread exactly
as pattern combinators build `Pat`; `VParam` mirrors `PParam`, so
late-bound vars, tweak slots, signals, number patterns (sampled at the
current cycle position), and functions of time all stay live per frame.

### 9.2 Shader compilation and swap

`compile_tex(&TexNode) -> Result<(ShaderDesc, UniformPlan), Failure>`
runs on the evaluator thread and generates fragment-shader source
(GLSL ES 3.0 for WebGL2 in v1) from a fixed per-operator snippet
library — the Hydra model. The result is SPLIT at the render
boundary:

```rust
pub struct ShaderDesc {                     // render-safe: plain data only
    source: String,                         // shader text
    uniform_names: Box<[String]>,           // binding order
    assets: Box<[TextAsset]> }              // non-numeric source payloads
pub struct TextAsset { id: u32, text: String }  // host rasterizes to a texture
pub struct UniformPlan { specs: Box<[UniformSpec]> }   // EVALUATOR-owned
pub struct UniformSpec { name: String, src: VParam }   // never crosses the boundary
pub struct Uniforms { values: Box<[f32]> }  // per frame, values only
```

`ShaderDesc` contains strings and plain metadata only — no `Value`,
no `VParam`, no `Rc`-backed evaluator state — so handing it to
`RenderHost` respects the values-only invariant (17, which admits
POD, `f32` uniforms, and plain-string descriptors); the host
rasterizes each `TextAsset` (the decided `text "hello"` source, whose
payload now lives in `TexKind::Text`) to a texture and binds it as a
sampler. `UniformPlan` stays on the evaluator side, attached to the
slot's evaluator state. A codegen, compile, or link failure is a
diagnostic with the chain's origin and the PREVIOUS program keeps
rendering — the visual analogue of the audio dry run. **Activation
timing is the slot table's, uniformly**: the program is compiled and
linked immediately, but the binding ACTIVATES at the next cycle
boundary through the same `pending` mechanism as every other slot
(11.2). The "next frame" activation of an earlier draft is withdrawn;
there is exactly one activation contract.

### 9.3 Per-frame evaluation and outputs

Each render tick (`requestAnimationFrame` or the `use-fps` timer) the
evaluator resolves every `UniformSpec` of the slot's evaluator-owned
`UniformPlan` to `f32` (time, signals, var derefs, sampled patterns)
and ships only the resulting `Uniforms` values through
`RenderHost::set_uniforms` — the render thread receives values only
and never runs closures. Outputs
`o0..o3` each own ping-pong framebuffers, so `src o1` feedback samples
the previous frame; `render oN` selects the displayed output and
`render` tiles all four. `use-fps`/`use-canvas` are session calls onto
`RenderHost` configuration. `hush`/`stop` on a visual slot releases the
program and clears the output to black (one slot table, 11.2).

### 9.4 Delivery scope in v1

The WebGL2 `RenderHost` lives in the editor's TypeScript shell and
serves both the browser and the Tauri webview; the headless native
binary and the REPL use `NoopRender` (visual chains still validate and
compile-check, but do not display). A native wgpu host is a later
follow-up, not in this plan. Planned tests (TASK-006, TASK-010): golden
shader source for `osc 20 > rotate 0.5 > out o0`; a
`text "hello" > out o1` golden including its `TextAsset`; a structural
check that `ShaderDesc` serializes as plain data (no evaluator-owned
payload can cross the boundary); time-varying uniform resolution from
the `UniformPlan`; output replacement activating at the cycle
boundary; hush/stop clearing; a failing chain leaves the previous
program rendering with a diagnostic.

## 10. Pattern Engine

### 10.1 Representation

A pattern is a function of time to events (Tidal model), lazy and
infinite. Representation is an inspectable node tree, not an opaque
closure, because the dry run, the editor's step highlighting, and
controller binding all need to SEE structure:

```rust
pub struct Pat { node: PatNode, span: Option<Span>, structured: bool }
pub enum PatNode {
    Steps(Box<[Step]>),                        // one cycle; nested = subdivide
    Sound { src: PParam, kit: Option<PParam> },  // every `s` call; `kit:` if given (below)
    Signal(Rc<Sig>),
    Fast(Rc<Pat>, PParam), Slow(Rc<Pat>, PParam), Rev(Rc<Pat>),
    Every(PParam, Value /*fn*/, Rc<Pat>), WhenMod(PParam, PParam, Value, Rc<Pat>),
    SometimesBy(PParam, Value, Rc<Pat>), DegradeBy(Rc<Pat>, PParam),
    Stack(Box<[Pat]>), Cat(Box<[Pat]>), FastCat(Box<[Pat]>),
    Superimpose(Rc<Pat>, Value), Off(Rc<Pat>, PParam, Value), Jux(Rc<Pat>, Value),
    Iter(Rc<Pat>, PParam), Chop(Rc<Pat>, PParam), Ply(Rc<Pat>, PParam),
    Striate(Rc<Pat>, PParam),                 // interleave n regions across events
    Slice { pat: Rc<Pat>, cuts: SliceCuts, index: Rc<Pat> },
    Splice { pat: Rc<Pat>, cuts: SliceCuts, index: Rc<Pat> }, // + rate-fit to step
    LoopAt(Rc<Pat>, PParam), Fit(Rc<Pat>),    // stretch over n cycles / to the event
    Chunk(Rc<Pat>, PParam, Value), Grid(Rc<Pat>, Rc<Pat>),   // `grid`, Tidal's `struct` (20 Q3, Decided)
    Euclid(Rc<Pat>, PParam, PParam, PParam),
    Control(KwId, Rc<Pat>, Rc<Pat>),           // gain/lpf/…: value pattern onto subject
    ScaleNotes(KwId, KwId, Rc<Pat>), Chord(…), Voicing(…), Arp(…),
    Segment(Rc<Pat>, PParam), Range(Rc<Pat>, PParam, PParam),
    MidiNotes { subject: Rc<Pat>, channel: Option<u8> },   // `s x > midi-notes`: live MIDI note input (11.7): yields NO
        // events under pure query/dry run. The bind-time INPUT-LANE walk
        // classifies operators between this node and the sink: control and
        // per-note probabilistic nodes apply per arriving note in tree order;
        // structural/time operators over the lane are a bind-time diagnostic,
        // never a silent bypass. Patterns remain the only event source: this
        // IS a pattern node, realized live instead of by lookahead.
}
pub enum PParam { Const(Value), Late(VarSlotRef), Fn(Value), Pat(Rc<Pat>) }
```

`Step` is `Value` plus its originating `SrcRef` (10.2); `nil` is a rest; a nested list
subdivides; `alt`/`maybe`/`euclid`/`hold`/`repeat`/`choose` inside a
step list are constructor calls that produce step-level nodes. A plain
list becomes `Steps` only where a pattern is expected (by type at that
position); everywhere else it stays an ordinary list — conversion is a
prelude/compiler coercion, never a mutation of the list.

`PParam::Late` and `PParam::Fn` are how late-bound `var`/`fn` names and
functions-of-time enter patterns (section 5.6): stored as references,
evaluated per query on the evaluator thread. EVERY numeric combinator
parameter — including the probabilities of `maybe`, `degrade-by`, and
`sometimes-by` — is a `PParam`, never a bare float, so tweak slots and
vars stay live inside probabilistic combinators (section 13).

**Sound first and the first-structure rule (Decided 2026-09-25,
design-music.md section 1).** A pattern chain starts with `s`; the
checker rejects a source before it (`sound-not-first`, 7.1.4).
`Pat::structured` is set at construction and is read only by the
structure-giving steps below; queries never read it.
- Every `s` call builds `Sound { src, kit }`. `kit` is the `kit:`
  argument as a `PParam` (`Const`, or `Late` for a `var`), or `None`.
  With ONE sound (a keyword, a `sound` value, or a late ref to one),
  `src` is `Const`/`Late` and the node is UNSTRUCTURED. With `s [..]`,
  `s {alt ..}` or `s {choose ..}`, `src` is `PParam::Pat` over the
  steps, and the node is structured from the start, with the steps'
  events.
- Per query, each keyword that `src` yields is looked up in the kit:
  the `kit` param's value when it is `Some`, otherwise
  `QueryVm::sound_kit()` (7.1.3). A `sound` value yielded by `src` is
  used as is. A missing key is an event-local `unknown-sound` failure
  (7.1.4). A structure-giving step keeps the resolved sound of its
  subject, so the kit travels with the chain.
- A STRUCTURE-GIVING step on an unstructured subject replaces the
  structure: a control whose value is list-valued or a structured
  pattern (`note [..]`, `n [..]`, `gain [..]`, ...), `euclid`, `grid`
  and `midi-notes`. Each result event takes its `whole`/`part` from the
  value pattern (or from the euclid/grid onsets, or the arriving note),
  and carries the sound plus every control the subject already has.
  The result is structured.
- A scalar or signal control on an unstructured subject leaves it
  unstructured and attaches the control; a signal is sampled at each
  event's onset once the structure is known.
- A control on a STRUCTURED subject keeps the subject's structure and
  samples the value pattern at each subject event's `whole.begin`
  (`part.begin` when `whole` is `None`); this is Tidal's `#`. So after
  the first list-valued step, later list controls are sampled at the
  existing onsets.
- Any other operator (`fast`, `every`, `rev`, `stack`, `off`, ...) on an
  unstructured subject, and a sink (`d1`..`d9`, `once`), realize it as
  one event per cycle with `whole = [c, c+1)`, Tidal's `pure`; the
  result is structured (question M4 in
  `design-docs/user-qa/pending-middle-end-questions.md`; this is what
  `s :crash > once` and `s :break > begin 0.25 > end 0.5 > d1` need).
- The rule applies only to chains built by `s`/`sound`. A list used
  where a value pattern is expected (a visual sequence, a control
  value) is structured by the list as before, and `tex/` chains are
  unchanged.
- Goldens (PATTERN wave): `s :pluck > note [:e2 :g2 :b2]` gives three
  events per cycle; `s [:bd :sn] > n [0 1 2 3]` gives two events with
  `n` 0 and 2; `s :pluck > gain [0.5 1] > note [:c :e :g]` gives two
  events with notes `:c` and `:e` (gain came first, so it gave the
  structure); `s :bd > euclid 3 8` gives three onsets; `s :bd > gain 0.5`
  gives one event per cycle.

**Sample-region operators (Decided: design-music.md "sample
slicing"; authority re-pinned in 13.5).** `SliceCuts` is
`Equal(PParam /*count*/)` or `Manual(Box<[PParam]> /*points*/)`.
Every operator compiles to REGION CONTROLS on the event — `begin`/
`end` (0..1 of the sample), `speed`, `loop` — via the existing
values-only control path (11.4; `SamplePlay` already honors
begin/end/speed/loop/cut, 12.1); no new audio-thread machinery.
PARTITION INVARIANCE — a TWO-LAYER contract. (The revision-12
claim that the continuation rule alone prevents duplicates under
OVERLAPPING or repeated windows is WITHDRAWN: Astra's [0,3/4) +
[1/4,1) counterexample shows an onset inside the overlap is
returned by both queries, as a pure stateless query must.)
LAYER 1 — QUERY COVER EQUIVALENCE (this section): each operator
is defined over the source event's WHOLE span — its stable
extent — never over the query-clipped `part`, and every
per-event region/speed value is a function of the source event's
OCCURRENCE IDENTITY (`occ`, 10.2). `query` is a pure function of
the span: for ANY cover of a span by subspans — adjacent,
overlapping, or repeated — the union of the results,
DEDUPLICATED BY `occ`, equals the whole-span result occurrence
for occurrence (same onsets, regions, rate-fit durations).
Overlapping windows each report the shared occurrences — they
return the SAME `occ` keys, never new occurrences. The standard
onset rule makes DISJOINT adjacent clipping neutral: a
(sub-)event is an ONSET (starts a note at commit) iff
`part.begin == whole.begin`; a clipped continuation
(`part.begin > whole.begin`) carries the SAME region controls as
its onset and never retriggers.
LAYER 2 — SCHEDULER EMISSION UNIQUENESS (11.3): at most one host
emission per occurrence per (slot, generation), and no staged
occurrence lost to query clipping. Deduplication is owned by the
scheduler, not the query layer — via per-occurrence RECORDS whose
covered extents fragments only EXTEND, scoped semantic
invalidation, and the committed-occurrence ledger, all defined in
11.3 — because only the scheduler sees every window it issued. A source event with
`whole = None` (a continuous fragment with no stable extent, e.g.
a signal-derived event) has no partition-invariant subdivision:
region operators record an EVENT-LOCAL fault for it (10.3)
instead of producing query-shape-dependent output.
- `chop k`: partitions each source event's WHOLE span into k
  equal exact-ratio sub-spans; child i takes that sub-span as its
  OWN `whole`, region i of k (contiguous, in order), and `part` =
  child whole INTERSECTED with the source part (empty
  intersection = child not emitted). Astra's counterexample now
  passes: one source event with whole [0,1) under `chop 2`,
  queried as [0,1/2) and [1/2,1) separately, yields exactly child
  whole [0,1/2) region 0 (onset 0) and child whole [1/2,1) region
  1 (onset 1/2) — the same two notes as the full-cycle query,
  never four. An OVERLAPPING re-query ([0,3/4) then [1/4,1))
  returns the shared child AGAIN — same `occ`, same onset 1/2 —
  and single emission is layer 2's job: the 11.3 occurrence
  merge commits it exactly once.
- `striate n`: timing unchanged; the slice ordinal derives from
  STABLE identity, never realization order inside the query
  window: the operator queries its source over each touched
  event's ENCLOSING CYCLE (query widening — one bounded extra
  source query per touched cycle, deterministic against the
  frozen read snapshot of 10.4), ranks that cycle's onsets by
  `whole.begin` (ties broken by stack/branch order, which is
  deterministic), assigns ordinal i by rank, region
  `[i mod n / n, (i mod n + 1) / n)`, then intersects each result
  with the requested span. The ordinal cannot restart at a query
  boundary because it does not depend on the query span.
- `slice cuts index-pat`: `Equal(n)` cuts at `i/n`; `Manual`
  points are slice STARTS — slice i spans `[p_i, p_{i+1})`, the
  last to 1.0. The index pattern picks the slice per event,
  SAMPLED AT THE EVENT'S `whole.begin` (identity-anchored: a
  clipped continuation reads the same index as its onset; timing
  unchanged). VALIDATION: count >= 1; manual points
  sorted strictly ascending within [0, 1]; literal violations are
  checker diagnostics, dynamic ones query-time event-local faults
  (10.3); an index outside `[0, slice count)` is an event-local
  fault, never a clamp.
- `splice`: `slice` plus rate fit — `speed` = slice seconds /
  WHOLE-SPAN seconds, resolved AT COMMIT (seconds exist only at
  the host boundary, 11.3/11.4) from the bank's sample duration
  in the `SampleLoader` manifest and the event's committed times;
  the intended extent is the event's whole span, NEVER an
  incidental query-clipped fragment, so a continuation carries
  its onset's speed.
- `loop-at n`: full region, `loop` = 1, `speed` = sample seconds
  / (n * cycle seconds) at commit, anchored at the whole span —
  the sample stretches over n cycles. `fit`: `speed` = sample
  seconds / WHOLE-SPAN seconds at commit. `cut n` stays the
  existing cut-group control.
All counts, points, and indices are `PParam`s/pattern literals —
numeric sites, editable from the waveform editor (13.5).

### 10.2 Time span and events

```rust
pub struct TimeSpan { begin: Ratio64, end: Ratio64 }        // cycles, exact
pub struct Event { whole: Option<TimeSpan>, part: TimeSpan, value: Value,
                   controls: Controls, src: Option<SrcRef>, occ: OccKey }
pub type Controls = BTreeMap<KwId, Value>;                  // sorted, small
pub struct OccKey { path: SmallVec<[(NodeId, u32); 8]>,     // structural ordinals
                    anchor: Ratio64, cycle: i64 }           // whole.begin, cycle
```

All positions are exact ratios (Tidal's `Rational`); a subdivided
sequence lands exactly on 0, 1/3, 2/3. Seconds appear only at the host
boundary. `occ` is the event's stable LOGICAL OCCURRENCE IDENTITY,
consumed ONLY by the scheduler's emission-uniqueness merge (11.3;
it carries no other semantics). `path` accumulates, during query,
one `(node id, ordinal)` entry per structure-introducing node the
event passed through — `Stack`/`Superimpose`/`Off`/`Jux` branch
index, `Cat`/`FastCat` child, `chop`/`striate` child ordinal, `Ply`
repetition, `Euclid` position — so two IDENTICAL simultaneous notes
from a two-branch stack carry DISTINCT keys (branch ordinals 0 and
1) and both are real. `anchor` is `whole.begin` (for `whole = None`
fragments, `part.begin`) and `cycle` the enclosing cycle. The same
logical note returned by different query windows carries an EQUAL
key — only `part` differs, by clipping — and `occ` is
deterministic against the frozen read snapshot (10.4). `src` is the ORIGINATING `SrcRef` of the step literal that
produced the event — element span plus the form generation and
`doc_revision` it was written under, copied from `ListProv` at
pattern coercion and carried end to end (`Step` stores it too). It is
the editor's highlight key, and it stays correct when a list literal
from one form and document revision is played by a binding evaluated
under another.

### 10.3 Query

```rust
pub struct QueryResult { events: Vec<Event>, faults: Vec<Failure>,
                         output: Vec<(Origin, Rc<str>)> }   // captured print, 10.4
pub fn query(p: &Pat, span: TimeSpan, cx: &mut QueryCtx) -> QueryResult;
```

Implemented by match over `PatNode`; `QueryCtx` carries the cycle
number, the pure per-(seed, node id, cycle) hash RNG (`rand`, `choose`,
`maybe`, `degrade-by` are repeatable with NO mutable RNG state), and a
VM handle in Query effect mode (10.4) for evaluating `PParam::Fn` and
pattern-transform closures (`every 4 {p -> fast p 2}` calls the closure
with the sub-pattern value and expects a pattern back).

**Failure granularity — one contract, shared verbatim with the plan:**

- **Event-local**: evaluating a per-event `PParam` or control fails —
  that event alone is removed, a `Failure` with full origin (span,
  slot, beat) is appended to `faults`, and sibling events in the same
  span survive.
- **Subtree-local**: a structural closure fails (`every`'s transform, a
  `chunk` function) — the failing child contributes no events for this
  span, one fault is recorded, and sibling branches of `Stack`/`Cat`/
  `Superimpose`/`Off`/`Jux` still produce theirs.
- **There is no query-wide failure**: `query` never returns an error;
  the worst case is empty `events` with `faults` explaining why.
  Multiple faults per query are retained, each with its own origin.

The scheduler forwards `faults` to the diagnostics bus; a slot's
runtime diagnostics clear when a full cycle queries and commits with
zero faults (8.4, 11.6). Planned test (TASK-006/007): a span with mixed
valid and failing events keeps the valid events audible, reports each
fault with slot and beat, and clears after a clean cycle.

### 10.4 Purity

Querying a cycle has no side effects (architecture guarantee), and the
VM ENFORCES it rather than trusting a warning. Closures evaluated
during query or dry run execute with `Vm.effect_mode = Query`, set and
restored by a scope guard so an unwinding failure cannot leave the
mode stuck. In Query mode the following fail immediately with a
`Failure` (code `effect-in-query`, full origin), handled by the
granularity rules above: `upd` on a global (`StoreGlobal`), slot
binds, `once`/`at`/`hush`/`stop`, tempo and canvas configuration, and
every host capability send (audio, MIDI, OSC, render, sample
loading). Reads of globals and tweak slots and frame-local
`let`/`var`/`upd` remain legal. `print` in Query mode performs NO
host write — console output is an effect, whether or not it changes
state — and is instead CAPTURED into `QueryResult.output` with its
origin. A dry run keeps captured output inside its report, so it
never reaches the live console. For live lookahead queries, captured
output is NOT forwarded when the span is first staged — staged spans
are revocable, and console text is not. Instead each captured line is
held in the staging record keyed by its event's identity (slot, exact
`Ratio64` onset, and source origin) and forwarded to the console only
at the span's COMMIT point (11.3 step 4), the same irreversible moment
its events become POD. Re-querying an uncommitted span (a control
write, 11.3) simply replaces that span's still-held, still-uncommitted
output before it is ever emitted, so restaging cannot duplicate a
print and cannot emit output for work that a later edit removes before
commit. Output whose event fails to commit (dropped at commit, 10.3)
is discarded with the event. Committed output is irreversible, exactly
like the sound. The static purity warning (section 7) is the
advisory complement of this hard rule.

**Determinism is relative to a read snapshot, not absolute.** Query
performs no writes; its result is a pure function of (pattern
structure, span, session seed, and the values read during the query
through `Late` refs, tweak slots, and host analysis cells). Repeating
a query with an unchanged read set is bit-identical; when a global or
analysis input changed between queries, the results differ exactly
through those reads — that is late binding working, not a purity
violation. Planned tests (TASK-005, TASK-007): `upd`-in-query fails
with origin while playback continues; a queried `print` appears in
the dry-run report and NOT on the live console; for live queries a
`print` reaches the console exactly once, AT its event's commit point,
with a control write that restages the still-uncommitted span BEFORE
commit replacing the held output (no duplicate, no emission for the
removed work) and a control write AFTER commit leaving the already-
emitted output untouched while affecting only later events;
repeated-span queries with a frozen read set are identical, and
varying one global changes results only through its recorded read;
failed AND successful dry runs leave namespace, slots, staging,
pending queues, and host outputs untouched; Query mode is restored
after an unwinding failure.

### 10.5 Signals

```rust
pub enum Sig { Sine, Saw, Tri, Square, Rand, Perlin, IRand(i64),
               Time, Beat, Phase, Cycle,
               Host(HostSig),          // fft band, amp, env — host analysis
               Cc { controller: u8, channel: u8 },  // MIDI CC input, 0..1 (11.7)
               Analyzer(AnalyzerId),   // level/spectrum/… cells (12.5), editor feedback
               Ctrl(KwId, KwId), Hits(KwId), // per-slot event signals (infra here;
                                             // visual-chain use deferred with coupling)
               Lag(Rc<Sig>, f64), MapRange(Rc<Sig>, …) }
```

A signal is continuous: queried at a point (event onset, control rate,
or per frame) rather than over a span. `segment` samples a signal into
events. Host signals read the latest analysis values published by the
audio host (plain `f32` cells written outside the audio callback's hot
path, read on the evaluator thread; single-writer so ordinary atomics
suffice). `Hits`/`Ctrl` read scheduler telemetry (section 11.6) — the
same stream that feeds editor highlighting; wiring them into VISUAL
chains stays deferred with music-visual coupling.

## 11. Clock, Scheduler, Slots

### 11.1 Cycle clock and ratio time

```rust
pub struct Tempo { bpm: Ratio64, beats_per_cycle: Ratio64 }   // cps derived
pub enum ClockSource { Internal, MidiClock, Link }            // :link represented,
                                                              // planned-only (a
                                                              // "not available"
                                                              // diagnostic in v1)
pub struct Clock { source: ClockSource, tempo: Tempo,
                   epoch_host: f64,                            // host seconds at cycle 0
                   pos: Ratio64 }                              // cycles, exact
```

`use-bpm` / `use-cycle` update `Tempo`; cps = bpm / 60 / beats_per_cycle,
kept as an exact ratio. Logical time (cycles, beats, step positions) is
`Ratio64` end to end through scheduler and pattern engine; conversion
to seconds (`f64`) happens once per event at the host boundary. Tempo
changes take effect from the current position (the clock is piecewise
linear: it stores the (host_seconds, cycle_pos) anchor at each change,
so past events never move). `use-clock` selects the source: under
`:midi` the clock is SLAVED to incoming MIDI clock and transport
(11.7) and `use-bpm` becomes a diagnostic ("clock is external");
`midi-clock-out true` makes the internal clock EMIT MIDI clock and
start/stop through `MidiHost` regardless of source.

### 11.2 Slot table

ONE table for sound and visual sinks (decided in `design-visual.md`):
`d1`..`d9` are `slot 1`..`slot 9`; `out o0`..`o3` bind visual slots;
`slot :drums` names one. `hush` silences every slot regardless of kind;
`stop :drums` stops one.

```rust
pub struct SlotId(u32);
pub enum SlotKind { Audio, Visual, Midi(u8), Osc(Rc<str>) }
pub enum Binding { Pattern(Rc<Pat>), Texture(Rc<TexNode>) }     // section 9
pub struct Slot { key: KwId, kind: SlotKind, bound: Option<Binding>,
                  pending: Option<Binding>,      // swap at next cycle boundary
                  gen: u32,                      // bumped on rebind/stop/hush (11.3)
                  orbit: OrbitId, muted: bool }
pub struct SlotTable { slots: Vec<Slot>, by_key: HashMap<KwId, SlotId> }
```

Binding (`d1`, `slot`, `out`) runs the dry run (11.5); on success the
new pattern goes to `pending` and the scheduler swaps it in at the next
cycle boundary, so a running phrase is never cut (decided). The sink
returns the pattern it bound, so chains continue (`> d1 > fast 2 > d2`).
`once` binds an ephemeral slot for exactly one iteration of the
pattern, starting now or at the `at:` beat offset; `at n: block`
schedules the thunk to run on the evaluator thread at that beat.

### 11.3 Scheduler loop

Runs on the evaluator thread, driven by a host tick (native: a timer
thread posts wake messages; wasm: the AudioWorklet posts its frame time
each render quantum — section 16). TWO horizons separate cheap
revocation from committed host events:

- **Query horizon** (`lookahead`, default ~120 ms as cycles): how far
  ahead patterns are queried. Queried events sit in an evaluator-side
  STAGING buffer per slot, still holding `Value`-level controls
  (`VarRef`/tweak refs unresolved). Staged events are revocable for
  free.
- **Commit horizon** (`commit_lead`, default ~30 ms: one host tick plus
  a transport-jitter margin): only events due within it are converted
  to POD (11.4) — late-bound controls resolve AT COMMIT — timestamped
  in host seconds, stamped with `(SlotId, gen)`, and pushed to the
  audio ring / MIDI / OSC queues. Committed events are immutable.

Each tick: (1) advance `pos`; (2) activate `pending` bindings whose
boundary has been crossed (bookkeeping only — prospective activation
below has already queried them); (3) query newly uncovered spans,
SPLITTING every query at cycle boundaries so no span crosses one; for
a slot with a `pending` binding, spans BEFORE the boundary query the
OLD binding and spans AT OR AFTER it query the NEW one — prospective
activation, so the new pattern's first events exist in staging before
the boundary and receive the full commit lead (layer 1 of the 10.1
partition-invariance contract keeps these split and incremental
queries semantically neutral — staging a span in pieces yields the
same occurrences as staging it whole — and the OCCURRENCE MERGE
below supplies layer 2, emission uniqueness); (4) commit staged
events entering the commit horizon, resolving late-bound control
VALUES and, at the SAME irreversible point, forwarding each committed
event's held captured `print` output to the console (10.4) — a
per-event resolution failure drops only that event, and its held
output, with an origin-carrying fault (10.3); (5) forward query faults
and publish telemetry (11.6); (6) run due `at`/`once` thunks (Normal
effect mode). Query faults are diagnostics, not console output, and
are forwarded immediately; only `print` output is deferred to commit.

**Occurrence merge — emission uniqueness (layer 2 of the 10.1
contract).** The scheduler guarantees AT MOST ONE host emission per
`occ` per (slot, generation) AND never loses a staged occurrence to
query clipping. The round-13 whole-record replacement rule is
WITHDRAWN: Astra showed a later query returning only a CONTINUATION
of an already-staged onset would replace — and thereby erase — the
onset-bearing entry. The corrected representation merges FRAGMENTS:

- **Occurrence records, independent of query clipping.** Staging
  holds exactly ONE record per `occ` per (slot, gen): the
  occurrence's `whole`, its identity-derived value/controls/`src`
  (equal across fragments by layer 1 UNDER ONE READ SNAPSHOT — a
  snapshot change reaches staging only through semantic
  invalidation below, which replaces payloads), and its COVERED
  EXTENT — the
  union of the `part` fragments received so far, kept as a small
  coalesced interval list within `whole` (bounded by the horizon's
  split granularity). A record is ONSET-BEARING iff its covered
  extent includes `whole.begin`. Emission is per RECORD, not per
  fragment: one voice start when the onset enters the commit
  horizon; fragments only extend what is known/coverable and feed
  invalidation bookkeeping and visual telemetry.
- **Coverage extension (the ONLY merge mode for ordinary query
  results — overlapping, adjacent, repeated, or disjoint windows
  alike).** An incoming fragment with an existing key UNIONS into
  the record's covered extent; it NEVER deletes coverage and never
  replaces the record. A continuation therefore cannot erase an
  uncommitted onset (it merely extends the record the onset lives
  in), and it cannot retrigger (emission is per record). Astra's
  case resolves: source whole [1,2) under `chop 2`, query [1,7/4)
  then [5/4,2) — or in the OPPOSITE order — yields two records
  (child wholes [1,3/2) and [3/2,2)); the second window's clipped
  child-0 fragment ([5/4,3/2)) merges into child 0's record, whose
  coverage still includes 1; onsets 1 and 3/2 each emit exactly
  once. NOTE the corrected scope of disjoint ownership: disjoint
  query spans prevent duplicate COVERAGE, not multiple fragments of
  one occurrence — an event crossing a span boundary legitimately
  arrives as an onset fragment in one span and continuation
  fragments in the next, and the record merges them into one
  emission. Step (3) still queries only newly uncovered spans; that
  discipline bounds work, it does not by itself deduplicate.
- **Semantic invalidation (the ONLY mode that removes — and the
  ONLY mode that REPLACES payloads).** A control write, tweak, or
  tempo-change re-commit invalidating uncommitted span S first
  DISCARDS coverage INSIDE S (a record wholly covered within S
  loses all coverage; a record merely clipped by S keeps its
  coverage outside S), then re-queries S under the NEW read
  snapshot and merges the results with one rule the
  extension-only mode deliberately lacks — SEMANTIC PAYLOAD
  REPLACEMENT (the round-14 gap Astra identified: coverage
  extension alone would re-attach coverage to a surviving record
  while keeping its OLD query-derived payload): for an
  uncommitted record whose `whole.begin` LIES IN S and whose key
  is PRESENT in the new result, the record's entire payload —
  `whole`, value, derived controls (a slice's begin/end region,
  a rate fit, provenance `src`) — is REPLACED by the new
  result's payload, so emission parameters come from EXACTLY ONE
  snapshot, the newest one covering the onset; retained
  out-of-span coverage is pure which-spans-are-known
  bookkeeping and carries no payload of its own (if the
  refreshed `whole.end` differs, coverage is intersected with
  the new whole; a changed `whole.begin` changes the OccKey and
  therefore takes the drop-plus-new-record path instead). The
  record's identity, generation, and single-emission accounting
  are untouched: same key, still at most one voice start, now
  with the refreshed controls. PARTIAL-INVALIDATION scope: a
  record whose `whole.begin` lies OUTSIDE S is NEVER
  payload-refreshed by S's re-query — invalidating only a
  continuation rebuilds that record's in-S coverage but does
  not rewrite its onset's payload, because the note's
  parameters are anchored at its onset — and committed
  occurrences stay immutable in every mode (the ledger blocks
  both re-emission and refresh). The absent-occurrence removal
  rule stays SCOPED to this mode: after the re-query of S, an
  uncommitted record whose `whole.begin` lies in S and whose
  key is absent from the new result is DROPPED; a record with
  `whole.begin` outside S is never dropped by S's re-query,
  only re-extended. Two identical simultaneous notes from a
  two-branch stack carry distinct keys (10.2) and are never
  collapsed in either mode. Worked example (Astra's
  slice-index case): an uncommitted occurrence with whole
  [1,2) under `slice` with two equal regions and a late index
  0 has region [0,1/2); `upd` the index to 1 and invalidate a
  span containing the onset — the re-query returns the SAME
  key with region [1/2,1); payload replacement installs the
  new region, and the recording host and renderer receive
  exactly one emission with region [1/2,1) and none with
  [0,1/2).
- **Committed ledger (the uniqueness backstop).** Commit (step 4)
  records each emitted `occ` in a per-(slot, gen) ledger bounded to
  the query horizon — an entry expires when `pos` passes its whole
  span, and the ledger drops with its generation at retirement. The
  merge marks any record whose key is already in the ledger as
  emitted (its onset can never re-emit), so a re-query overlapping
  already-committed work — e.g. [0,3/4) staged and partially
  committed, then [1/4,1) queried — never re-emits onset 1/2.
  Committed events stay immutable; a rebind's NEW generation
  legitimately starts a fresh ledger.

The corresponding regressions assert EMITTED MULTIPLICITY at a
recording host — each onset exactly once AND no onset lost: the
future-span case with both overlapping windows staged BEFORE any
commit (in both query orders), boundary-crossing fragments arriving
across disjoint adjacent spans (onset first and continuation first),
partial invalidation clipping a record without dropping its onset,
overlapped/repeated windows, the partially committed ledger case,
and BOTH branches of an identical two-note stack present — never
set equality, which would mask
duplicates and erasures alike.

Revocation and control semantics. The sink-side primitive is ONE
control message every sink implements:

```rust
pub struct SlotControl { slot: SlotId, new_gen: u32,
                         effective_time: f64, release: Release }
pub enum Release { None, Natural, Panic }     // rebind / stop / hush
```

A sink receiving `SlotControl` (a) drops queued events of that slot
whose generation is older than `new_gen` AND whose time is at or
after `effective_time`; (b) applies `release` to voices that STARTED
BEFORE `effective_time`: `None` lets them ring (rebind — the old
cycle's tail belongs to the old pattern), `Natural` lets them play to
their natural end (stop), `Panic` gates them with a short release, a
few ms (hush); and (c) — an ALWAYS-ON late-start recovery rule,
independent of `release` — short-gates any stale-generation voice
that started AT OR AFTER `effective_time`: such a voice exists only
because the control arrived after the event began, and it is cut the
moment the control lands. Sink-specific recovery: `MidiHost` sends an
immediate note-off for each stale-started note and all-notes-off on
`Panic`; `OscHost` applies revocation to its UNTRANSMITTED queue only
— an OSC message already sent to the network is irrevocable, stated
plainly, so OSC staleness is bounded by the untransmitted-queue rules
alone. All three sinks implement
`fn control(&mut self, c: SlotControl)` and acknowledge with
`SlotControlAck { slot, gen }` on their return path, and
`MidiEvent`/`OscEvent` carry `slot` and `gen` exactly as `AudioEvent`
does. Committed event batches also carry each slot's current `gen`;
what that piggyback can and cannot do is bounded precisely below
(narrow backstop bullet in the control-channel list): it enables only
the default drop/gate policy, never reconstruction of a lost
control's effective time or release.

- **Rebind (deadline-aware; three cases, not one unconditional
  guarantee).** Setting `pending` immediately (a) DISCARDS the slot's
  staged events at or beyond the next boundary — including events
  staged before the rebind arrived — and re-queries that region from
  the new binding (prospective activation), and (b) bumps the slot
  `gen` and sends `SlotControl { effective_time: boundary,
  Release::None }` so old-binding events past the boundary are
  revoked. Old events before the boundary play out: the old pattern
  owns the current cycle by the decided swap rule. What is
  ACHIEVABLE then depends on how much time remains before the target
  boundary `B` when the rebind is applied, relative to `commit_lead`
  and the one-hop control latency `L_ctl`:
  - **Sufficient lead** (`now + commit_lead + L_ctl <= B`): the new
    pattern's boundary events are committed with full lead and no old
    event sounds at or after `B`. This is the common case and the one
    the 0.98-rebind test asserts WITH an explicit cycle duration long
    enough to satisfy the inequality.
  - **Insufficient lead** (`B - now < commit_lead`, e.g. a 20 ms
    remainder against a 30 ms lead): the boundary events CANNOT have
    been committed 30 ms early — physics, not a design gap. The
    scheduler commits them as early as the remaining time allows
    (reduced lead, flagged in telemetry) rather than dropping them,
    and old committed boundary events are still revoked IF their
    `SlotControl` reaches the sink before their start time.
  - **Control-delivery failure** (the actual delivery delay `D`
    exceeds the remaining time to committed old events' starts): the
    one-event bound of an earlier draft is withdrawn — simultaneous
    events exist. The honest bound is DENSITY-DERIVED: every
    old-generation event whose start time falls in
    `[effective_time, effective_time + D)` may begin (their count is
    the slot's event density over that window — three simultaneous
    boundary events are three artifacts), and EACH such stale-started
    voice is cut by the always-on late-start recovery rule the moment
    the control (or the next gen-carrying event batch) lands, so each
    artifact lasts at most `D` plus the short gate. The session
    reports the missed deadline. Under REPEATED control loss no fixed
    bound holds and none is claimed: the re-send/ack loop below
    governs, the event-batch gen piggyback repairs the table as soon
    as any newer batch arrives, and persistent non-delivery raises a
    transport diagnostic. OSC messages already transmitted during the
    window are irrevocable (above).
  The unconditional "zero stale events past the boundary regardless of
  arrival time" claim is withdrawn; the guarantee is conditional on
  sufficient lead, and the degradation is density-bounded per delivery
  delay and reported.
- **stop / hush**: bump `gen` (hush: every slot), clear that staging,
  send `SlotControl { effective_time: now, release }` with `Natural`
  (stop) or `Panic` (hush). Per sink and per release class — never a
  blanket silence claim: NEW events cease within control-message
  delivery; hush's `Panic` gates sounding voices within it; stop's
  `Natural` lets sounding voices finish naturally (an open-duration
  input voice enters its release stage, 11.7); an OSC message
  already transmitted is irrevocable (above). Nothing waits for the
  lookahead window.
- **Tempo change** (`use-bpm`/`use-cycle`): re-anchor the clock
  (11.1), bump every slot `gen` with `effective_time: now`, clear
  staging, re-query and re-commit from the current position.
- **Control writes and STRUCTURE**: any `upd`/`set-var`/`set-tweak`
  reaching a live reference INVALIDATES the staging buffer (staged,
  uncommitted events) and the next tick re-queries the uncovered
  spans. Structural parameters — probabilities, `fast` factors,
  anything resolved at query time — therefore take effect for every
  uncommitted event, not merely control values: raising a `maybe`
  probability from 0 to 1 RESTORES events the previous staging had
  dropped (and, symmetrically, raising a `degrade-by` probability
  REMOVES more events) — either way the uncommitted topology is
  rebuilt, provided the affected events are not yet committed. Staging
  is a ~120 ms window of enforced-pure queries, so rebuilding it on
  session-coalesced (latest-wins per tick) controller input is cheap
  by construction. Control VALUES on staged events still resolve at
  commit, as above.
- **Value changes go through control cells with an explicit,
  per-tier TRANSPORT — no visibility is assumed across isolated
  memories.** Commit converts a late-bound control (a `VarRef`/tweak
  ref) into `Ctl::Cell(CellId)` rather than a frozen `f32`. The
  evaluator owns the AUTHORITATIVE cell table; the audio side reads a
  cell at VOICE START (and continuously for control-rate parameters).
  How a write becomes visible is tier-specific and stated as such:
  - NATIVE: evaluator and audio callback share one process; a cell is
    an atomic `f32` (release store / acquire load). A write is
    visible to the very next voice start — the decided "heard at the
    next event" holds to within one audio buffer.
  - BROWSER: the two Wasm instances have ISOLATED memories (16), so
    the worklet holds a MIRROR cell table, maintained by a BOUNDED,
    ORDERED, INITIALIZED protocol on the priority control port:
    - **Incarnation state machine (per id, mirror side).** A live
      cell is `(CellId, epoch)`; epochs per id are strictly
      increasing across the session (the evaluator allocates them;
      reuse waits for the retire ack). The mirror keeps, per id,
      `state in { Vacant, Live(epoch, value), Retiring(epoch) }`
      plus `last_epoch` (highest epoch ever seen, surviving
      Vacant). Exactly ONE message class is AUTHORIZED to change
      the incarnation: `CellInit { cell, epoch, value }` with
      `epoch > last_epoch` performs the Vacant -> Live transition
      (also Live/Retiring -> Live on a strictly greater epoch — a
      supersede, covering a lost retire ack). Every OTHER rule is
      epoch-checked against the CURRENT Live epoch: `CellUpdate`
      entries apply only in `Live` with an exactly matching epoch
      (a stale update after reuse is inert); `CellRetire(epoch)`
      moves the matching Live to Retiring, and the drained
      `Retired` ack moves Retiring to Vacant. The universal-
      rejection rule of the previous revision is REPLACED by this
      machine: initialization is the authorized transition, not an
      epoch-equality violation.
    - **Initialization is INIT-ONCE per epoch; a replay is
      acknowledge-only.** The mirror records, per id, the highest
      epoch it has INITIALIZED. A `CellInit` whose epoch equals an
      already-initialized epoch does NOT write the value — it only
      re-sends `CellInitAck { cell, epoch }` (retransmission
      safety: a delayed or re-sent `CellInit(E, v0)` arriving
      after a `CellBatch` set the cell to v1 leaves v1 in place;
      the previous equal-epoch last-write-wins rule is WITHDRAWN
      as Astra's replay counterexample). A `CellInit` with
      `epoch <= last_epoch` but not the initialized-live epoch is
      stale and dropped (ack-only with its own epoch, keeping the
      re-send loop idempotent). Value changes travel ONLY in
      batches; `CellInit` carries a value solely for the one
      Vacant/supersede transition, so init and updates need no
      shared version counter — the FIFO port orders them, and the
      init-once rule makes reordering-by-retransmission harmless.
    - **Initialization barrier — no uninitialized read, ever.**
      `CellInit` is sent when the owning site or `InstDef` is
      installed (bind/definition time, well before first use).
      The scheduler consults the ack state AT COMMIT: an event (or
      inst default) referencing a not-yet-acked incarnation is
      committed with that control DOWNGRADED to `Ctl::Const`
      carrying the evaluator's current value — the event is never
      delayed and the worklet never reads a cell it has not
      initialized; once the ack arrives, later commits use
      `Ctl::Cell` normally. `CellInit` re-sends under the standard
      unacked-threshold rule.
    - **Ordered, bounded batches.** Value writes are batched per
      tick as `CellUpdate { cell, epoch, value }` entries in a
      `CellBatch { seq }` (coalesced LATEST-WINS PER CELL — correct
      for values, unlike slot controls). `seq` is per-session
      monotone; the port is FIFO, and the mirror additionally
      rejects a batch whose `seq` is not greater than the last
      applied (duplicate/reorder defense). The sender holds at most
      TWO batches outstanding (one in flight unacked + ONE pending):
      while blocked, further writes coalesce into the single pending
      batch, whose size is bounded by the preallocated cell pool —
      a stalled worklet therefore never accumulates more than two
      batches of pending work, and memory is bounded by
      construction. The worklet applies at most one batch per
      `process()` (the same audio-progress credit discipline as
      16.1 installs; cell batches count against that quantum's
      handler budget) and acks `CellBatchAck { seq }`; credit
      replenishes only as audio advances.
    - **Failure and reconnect.** Unacked past the threshold =
      host-transport diagnostic (as for slot controls). On port
      re-establishment the evaluator first re-syncs the mirror,
      chunked under the 16.1 credit: `CellInit` ONLY for
      incarnations the mirror has never initialized (per the
      state machine), then ONE sequence-numbered snapshot batch
      stream carrying every live cell's current value — ordinary
      epoch-checked `CellUpdate` entries, which OVERWRITE values
      correctly (init-once applies to `CellInit`, not to batches),
      so values missed in lost batches are restored. Incremental
      batches resume after the snapshot acks; commits during
      re-sync use the `Const` downgrade above.
    The guarantee is DELIVERY-DEPENDENT and scoped to CELL-CARRIED
    controls: an event committed with `Ctl::Cell` hears a change
    once its batch is applied — in practice `upd` + one control
    hop `L_ctl`; an event starting inside that hop plays the
    previous value. **Const-fallback exception (explicit).** An
    event committed under the pre-ack `Const` downgrade — sites
    and cell-backed `InstDef` defaults alike — carries its
    COMMIT-TIME value for its whole life: if a newer batch applies
    before that event starts, the event still plays its committed
    constant. This is a stated exception to the batch-application
    guarantee, not covered by it; it is BOUNDED to the
    initialization window (install -> `CellInitAck`, re-sent under
    the unacked threshold) plus at most `commit_lead` of
    already-committed events, and the very next commit after the
    ack returns that control to the cell path. A FIRST use races
    nothing: it either reads the initialized mirror or carries the
    committed `Const` snapshot. No claim is made that an evaluator
    write is already visible in worklet memory.
  Cell identity and lifetime: `CellId`s come from a preallocated pool
  sized at init; a cell retires with its owning site/`InstDef`
  through the standard generation/refcount retirement (16.1), and an
  id is reused only after the worklet acks the retire, with the
  incarnation epoch above making retire acks idempotent and
  post-reuse stale updates inert. `InstDef` parameter defaults are
  `(CtlId, Ctl)` — `Const` or `Cell` — reconciling the inst-default
  slider path (13) with this representation; a cell-backed default
  is initialized (and ack-gated) at `inst` install exactly like a
  site cell. Constant controls stay baked `f32`. The rev-4
  commitment framing remains withdrawn. THREE
  narrow deviations are FLAGGED FOR ADJUDICATION rather than
  reinterpreted: (i) the browser cell hop above; (ii)
  `MidiHost`/`OscHost` bake values at transmission (bounded by
  `commit_lead`); (iii) STRUCTURAL changes rebuild
  staged-but-uncommitted topology only.
- **Control channel: two ordered classes per slot; the immediate
  class MERGES monotonically, never replaces** (extends section 18):
  `SlotControl` messages ride a SEPARATE priority control queue per
  sink, never the event ring, so a full ring cannot block a
  revocation. Per slot the session holds at most TWO outstanding
  entries — one IMMEDIATE-effective and one FUTURE-effective (a
  rebind at a boundary) — delivered in `effective_time` order.
  Same-class replacement is WITHDRAWN for the immediate class because
  it could discard a Panic (hush-then-stop would downgrade to
  Natural): the immediate entry is a MONOTONE MERGE of every
  immediate control since the last ack — `new_gen` = max,
  `effective_time` = min, `release` = maximum severity on the lattice
  `None < Natural < Panic`. Hush-then-stop therefore keeps Panic;
  hush-then-tempo keeps Panic while the tempo's gen bump rides the
  same merged entry; hush-then-rebind keeps both entries (immediate
  Panic AND the future boundary activation). The merge is monotone,
  so re-sending until `SlotControlAck { slot, gen }` returns is
  idempotent (a duplicate application changes nothing); the future
  entry still replaces only itself (a newer rebind supersedes an
  older unactivated one). After a threshold of unacked ticks the
  session raises a host-transport diagnostic (browser: attempts port
  re-establishment).
- **The gen piggyback is a NARROW backstop, not control
  reconstruction.** Committed event batches carry each slot's
  current `gen`; from that alone the sink can only apply the DEFAULT
  conservative policy from the moment of receipt: drop
  older-generation queued events at dequeue and short-gate
  stale-started voices. It canNOT reconstruct a lost control's
  `effective_time` (a boundary-scoped rebind drop) or its `release`
  policy — old events BEFORE a lost rebind's boundary are also
  dropped under the default policy (over-eager but safe), and a
  Panic that existed only in the lost message is not applied until
  the re-send loop delivers it. The full revocation semantics
  require the control message itself; the piggyback merely bounds
  stale playback while re-send completes. The earlier "repairs lost
  controls" phrasing is narrowed accordingly.
- Slot rebinds and `reeval`-tier rebuilds (5.6, 13) land at the next
  cycle boundary, per the decided swap rule.

Planned deterministic tests (TASK-007; mock clock + recording hosts,
explicit cycle duration and transport delay per case): each command
arriving (a) after staging and (b) after commitment — rebind (all
three lead cases), stop, hush, set-tweak (control value AND
structural probability, `maybe` 0 -> 1 restoring previously absent
events and `degrade-by` 0 -> 1 removing events), tempo change —
produces exactly the bounded behavior above, across cycle
boundaries, `once`/`at`, and the audio, MIDI, and OSC sinks alike; a
SUFFICIENT-lead rebind (`B - now >= commit_lead + L_ctl` by chosen
cycle duration) yields zero old-pattern events past the boundary and
full-lead new boundary events; THREE SIMULTANEOUS old-generation
boundary events with a delayed control all start and are all cut on
control receipt (density-bounded artifact, each within `D` + gate);
HUSH-NOW THEN REBIND-AT-BOUNDARY delivered before either lands
preserves both actions in order (immediate silence AND boundary
activation); REPEATED control loss exercises re-send/ack, the
event-batch gen piggyback repair, and the transport diagnostic
threshold; `upd` on a cell-referenced control is heard at the next
STARTED event even when that event was already committed (the
decided next-event rule — asserted natively; on the browser model,
after the batch applies, per the delivery-dependent guarantee); a
FIRST event referencing a new cell incarnation BEFORE `CellInitAck`
commits with the `Const` downgrade and sounds the correct value,
and after the ack the cell path resumes; an INIT REPLAY after an
update (`CellInit(E, v0)` retransmitted after a batch set v1) is
acknowledge-only and leaves v1 in place; a RETIRED-then-REUSED id
initializes its new epoch through the Vacant -> Live transition
and old-epoch updates stay inert; a PRE-ACK `Const` commit whose
event starts AFTER the ack and a newer batch still plays its
committed constant — asserted as the documented Const-fallback
exception, with the next commit returning to the cell path; a
STALLED-CONSUMER BURST
(many writes while the mirror applies nothing) never exceeds two
outstanding batches, coalesces latest-wins into the pending batch,
and on resume the mirror equals the latest evaluator values; a
`CellUpdate` carrying a RETIRED epoch after id reuse is dropped by
the mirror (no write to the new owner); a reconnect replays the
full snapshot before any new voice start reads the mirror; a MIDI
stale-started note receives an immediate note-off; OSC revocation
applies only to the untransmitted queue.

### 11.4 Event and control handoff (POD only)

```rust
#[repr(C)] pub struct AudioEvent { time: f64, slot: SlotId, gen: u32,
                                   inst: InstId, voice_hint: u32,
                                   n_ctl: u8, ctl: [(CtlId, Ctl); MAX_CTLS] }
#[repr(C)] pub enum Ctl { Const(f32), Cell(CellId) }  // Cell: late-bound value,
    // read from the control-cell table at voice start (control-rate params:
    // continuously) via the PER-TIER transport of 11.3 — native: atomic f32
    // in shared process memory (next-event holds to one buffer); browser:
    // worklet mirror updated by CellUpdate batches, delivery-dependent
pub struct CtlId(u16);   // interned control name -> dense id (gain, note, lpf, …)
```

Fixed-size, `Copy`, no `Rc`, no `Value` — the audio thread never sees a
language value or runs a closure. Events carry their slot and
generation so sinks can drop revoked work (11.3); control values are
resolved at commit time, never earlier. Control mapping follows
`design-music.md`: `note`/`n` through the scale to `freq`, `gain` to the
instrument's `amp` (naming: section 20 Q4), any other control by name;
effects (room, size, delay, crush, shape, vowel, …) are controls routed
to the slot's orbit effect unit. MIDI sinks get
`MidiEvent { time, slot, gen, ch, note, vel, dur }`; OSC sinks get
`OscEvent { time, slot, gen, addr, args: SmallVec<OscArg> }` — every
sink shares the slot/generation identity and the `SlotControl`
revocation contract of 11.3.

### 11.5 Capability traits, no-op host, dry run

```rust
pub trait AudioHost  { fn send(&mut self, ev: AudioEvent);
                       fn control(&mut self, c: SlotControl);        // 11.3
                       fn now(&self) -> f64;
                       fn swap_graph(&mut self, g: GraphHandle); fn analysis(&self) -> HostSigs; }
pub trait RenderHost { fn set_program(&mut self, o: OutId, sh: ShaderDesc);
                       fn set_uniforms(&mut self, o: OutId, u: &Uniforms); }
pub trait MidiHost / OscHost / ConsoleHost / SampleLoader { … }
// MidiHost and OscHost also implement fn control(&mut self, c: SlotControl)
```

The core depends only on these traits (Wasm constraint: all platform
access via capabilities). `NoopHost` implements them all as no-ops and
is the **dry-run** target: on every bind, the session queries one full
cycle of the new binding against `NoopHost`, in Query effect mode
(10.4), over a read view of the current namespace —
`dry_run(&Binding, &Session) -> QueryResult`. Query mode makes the
isolation ENFORCED, not assumed: global writes, scheduling, and host
sends fail inside the dry run instead of escaping it, and the pure hash
RNG leaves no state behind, so both successful and failed dry runs
leave namespace, slots, staging, pending queues, and host outputs
unchanged (tested, TASK-007). Any fault becomes a diagnostic, captured
`print` output stays inside the dry-run report (10.4) and never
reaches the live console, and the old binding keeps playing. Because
nothing suspends and query purity is enforced, one cycle is a complete
check — deterministic relative to the read snapshot taken during it.

### 11.6 Telemetry (playing-state feedback)

The scheduler publishes, per realized event:

```rust
pub struct PlayingEvent { slot: KwId, beat: Ratio64, time: f64,
                          src: Option<SrcRef>, dur: f64, kind: SlotKind }
```

into a bounded queue drained by the session layer, which forwards it to
subscribed editors (section 14) — this is what highlights the step that
is sounding — and retains a rolling window that backs the `hits`/`ctrl`
event signals and per-slot level meters. Events carry their own
originating `SrcRef` end to end (10.2); the session forwards it
UNCHANGED, and only an event without provenance falls back to the
binding form's `SrcRef` — so a stored list playing under a later
document revision still maps against the revision it was written in
(14.4). Nothing re-wraps provenance with the owning form's identity.

### 11.7 MIDI input, clock and transport (Decided scope)

`MidiInHost` is the input capability (Web MIDI in the browser,
CoreMIDI natively; absent = "not available" diagnostic):

```rust
pub trait MidiInHost { fn poll(&mut self) -> &[MidiInEvent]; }   // drained per tick
pub enum MidiInEvent { Cc { ch: u8, controller: u8, value: u8, time: f64 },
                       NoteOn { ch: u8, note: u8, vel: u8, time: f64 },
                       NoteOff { ch: u8, note: u8, time: f64 },
                       Clock { time: f64 }, Start, Stop, Continue }
```

Input events arrive on the IO thread and are drained by the SESSION
each tick on the evaluator thread — user code never runs off it.

- **`cc n channel:`** is a SIGNAL (10.5 `Sig::Cc`): the session
  maintains a per-(channel, controller) `f32` cell array (0..1);
  draining writes cells; the signal reads the cell at query/frame
  time like any host signal. The same cells feed MIDI-learn (13).
- **`midi-notes channel:`** is the `PatNode::MidiNotes` LIVE pattern
  (10.1). It is a structure-giving STEP after `s` (Decided 2026-09-25,
  design-music "sources and destinations"): `s :pluck > midi-notes
  channel: 1 > d1`. The subject is the destination sound with the
  controls attached so far; each arriving note becomes one event of
  that sound carrying the note. The subject must be unstructured (a
  single sound, 10.1): a structured subject (`s [:a :b] > midi-notes`)
  would re-time live input, so the bind-time walk below reports it as
  `input-lane-operator` too. Under pure query and dry run it yields no
  events (future input is unknowable; purity is preserved).
  COMPOSITION is defined by an INPUT-LANE partition performed AT BIND
  TIME: the dry run walks the tree from each `MidiNotes` node to the
  sink and classifies every operator on that path (the operators
  AFTER `midi-notes` in the chain) —
  - SUPPORTED live operators, applied PER ARRIVING NOTE in tree
    order: control-attaching nodes (`Control`, `ScaleNotes`, chords
    and their kin — they decorate the note event) and per-event
    probabilistic filters (`maybe`, `degrade-by`, `sometimes-by`
    with a per-event closure: the note is kept or dropped using the
    seeded RNG keyed by the node id and the note's arrival sequence
    number, and a kept note passes through the transform). Binding
    `s :pluck > midi-notes > degrade-by 1 > d1` therefore drops
    EVERY note — filtering is applied on the live path, never
    bypassed.
  - UNSUPPORTED over live input: structural and time operators that
    re-time, replicate, or need lookahead (`fast`/`slow`/`rev`/
    `every`/`off`/`iter`/`chop`/`ply`/`chunk`/`euclid`/`segment`,
    `cat`/`fastcat` re-sequencing of the input lane itself). These
    are a BIND-TIME DIAGNOSTIC ("cannot re-time live input") from
    the same dry-run walk — the bind is rejected and the old binding
    keeps playing; nothing is silently bypassed. `stack` of an input
    lane WITH ordinary pattern branches is supported: the input lane
    and the queried branches realize independently.
  NOTE LIFETIME (revised — release authority is keyed by INSTANCE
  identity, not by the current slot generation): the session records
  EVERY arriving NoteOn as a `NoteInstance { slot, gen, seq,
  filtered }` — `filtered` marks notes dropped by the input-lane
  probabilistic operators, recorded anyway so MIDI off-matching
  stays correct. A surviving NoteOn commits ONE event (bypassing
  staging — live input cannot be looked ahead) whose duration is
  OPEN, carrying `VoiceTag = (slot, channel, pitch, seq)`. ORDERED
  DELIVERY: a live NoteOn start and every `VoiceRelease` travel on
  the SAME FIFO priority control channel per sink (live input never
  uses the time-ordered event ring), so a release can never
  overtake its own start; as defense in depth the audio side keeps
  a small bounded TOMBSTONE ring of releases that matched no voice,
  and a NoteOn arriving with a tombstoned tag is dropped instead of
  starting an unreleasable voice. The audio side keeps a
  preallocated tag -> voice map; on pool exhaustion the OLDEST open
  input voice is stolen (short gate) and counted — a diagnostic,
  never an allocation. RELEASE: a NoteOff matches, session-side,
  the EARLIEST UNMATCHED NoteInstance of that (channel, pitch) —
  filtered or not, per MIDI convention; a filtered instance
  CONSUMES its NoteOff silently (no message), a surviving one sends
  `VoiceRelease { tag }`, which releases exactly the tagged voice
  (envelope release stage) IF IT IS STILL SOUNDING, REGARDLESS of
  any slot generation bump since it started — the tag names an old
  instance, so releasing it after a rebind is correct, and it
  cannot touch the new binding's voices because their tags differ
  (the earlier stale-gen no-op rule is WITHDRAWN as the stuck-note
  hazard Astra identified). Slot-control interaction for
  OPEN-duration voices: `Panic` short-gates them; `Natural` —
  whose meaning for a voice with no scheduled end is key-up —
  ENTERS THE RELEASE STAGE immediately (stop cannot leave an open
  note sustaining forever), and the session simultaneously closes
  its `NoteInstance` records for that slot so later NoteOffs for
  them are consumed silently; rebind's `None` lets open old-gen
  voices ring, still releasable by their own NoteOffs via tag as
  above. Latency = drain tick + commit path; quantization is a
  later option, not v1.
- **Clock slave (`use-clock :midi`)**: MIDI clock is 24 pulses per
  quarter note. The session timestamps pulses, low-pass-filters the
  inter-pulse period (simple exponential smoothing; jitter must not
  wobble `Ratio64` positions), and re-anchors the piecewise-linear
  clock continuously: logical positions stay exact ratios, only the
  host-seconds anchor tracks the external tempo. `Start` resets
  `pos` to the next cycle 0; `Stop` freezes the scheduler (staging
  cleared, `SlotControl` Natural to all sinks); `Continue` resumes
  from the frozen position. Loss of clock (no pulse within a
  timeout) freewheels at the last smoothed tempo and reports a
  diagnostic.
- **Clock master (`midi-clock-out true`)**: the scheduler emits
  Clock/Start/Stop through `MidiHost` from the internal clock with
  the same lookahead/commit discipline as note events.
- **Link** (`ClockSource::Link`): same slaving mechanism, different
  transport; represented in the enum, implementation planned-only,
  selection is a diagnostic in v1 (per the spec's "planned").

## 12. DSP Graph and the Audio Thread

### 12.1 Instrument definition to graph template

`inst` bodies are evaluated ONCE, at definition time, on the evaluator
thread; ugen functions (`saw`, `lpf`, `env-perc`, …) do not produce
audio — they build nodes. The result is a template:

```rust
pub struct InstDef { id: InstId, params: Box<[(CtlId, Ctl /*default:
                     Const or Cell — cell-backed defaults are the
                     inst-default slider sites, 11.4 and 13*/)]>,
                     nodes: Box<[UGenSpec]>, edges: Box<[Edge]> }
pub enum UGenSpec {
    // core (as before)
    SinOsc, Saw, Pulse, Tri, WhiteNoise, Lpf, Hpf, Bpf,
    Delay, Comb, EnvPerc, EnvAdsr, Line, SamplePlay(BankRef),
    Mul, Add, Const(f32), Param(CtlId),
    // synthesis-model ugens (design-music.md section 4, Decided)
    Vco { unison_max: u8 }, SubOsc, Ladder, Svf,
    FmOp, FmMod, PhaseDistortion, Additive { partials_max: u8 },
    Wavetable(TableRef), Granular(GranSrc),        // 12.6
    Effect(EffectSpec),                            // any builtin effect as a ugen (12.5)
}
```

A sample is an instrument the host provides: `s :bd-haus` resolves to a
`SamplePlay` template with the standard controls (speed, begin, end,
cut, legato, …).

### 12.2 Audio-thread execution

The audio callback (cpal stream / AudioWorklet `process`) owns:

- a **voice pool**, preallocated (`Vec<Voice>` fixed capacity; a voice
  is an instantiated template with per-node state buffers, allocated at
  graph-swap time on the evaluator/handoff path, never in the callback);
- per-orbit **effect units** (reverb, delay, bit/shape) with parameter
  smoothing;
- the **event ring** (lock-free SPSC): each callback drains events due
  within the buffer, starts voices sample-accurately, applies controls;
- **control cells**: latest control-rate values for function-of-time
  params, written by the evaluator, read by voices.

**Invariants: no allocation, no locks, no `Rc`, no closure evaluation
in the callback.** NATIVE builds hand graph/template swaps over a
triple-buffer of fully built structures; the retired structure goes to
a "garbage" queue and is dropped on the evaluator thread, and sample
data is `Arc<[f32]>` loaded by `SampleLoader` off the audio thread,
with the callback only reading. The browser cannot share Rust objects
between its two isolated Wasm memories; its equivalent protocol —
preallocated arenas, bounded chunked installs, acknowledgments, and
index-based retirement — is section 16.1.

### 12.3 Analysis

The host computes `amp` (RMS) and an FFT band vector on a low-priority
tap (native: a separate thread fed by a ring; wasm: an
`AnalyserNode`/worklet tap) and publishes `f32` cells the evaluator
reads for `fft n` / `amp` signals.

*Amendment (author, 2026-09-25 — self-analysis and loopback).* Every
analyzer names its SOURCE: a bus id (`:master`, a `bus :name`, a slot
`:d1`) or the host input `:in`. Bus taps are internal copies of the bus
signal on the analysis path (12.5 analyzer units), so the engine can
analyse its own output without any acoustic or device loop; input
monitoring is off unless requested. Two further sources of sample
values reuse the same taps: `capture :bus cycles` (record a bus into a
`SampleBuf`, the sampler's recording face) and `render cycles` (offline
render of the current bindings through `Engine::render`, native tier
only per 12.7 — browser reports "not available on this host"). Analysis
functions over a sample value (`rms`, `peak`, `spectrum bins:`,
`scope n`) run the same analyzer code on a buffer. This is also the
TEST path: headless tests render N cycles and assert on the numbers
(RMS, peak, band energies, zero crossings) instead of on raw sample
equality where a numeric criterion is the intent. Owner: the analyzer
units and `Engine::render` are TASK-008; the prelude functions and the
REPL/editor surfaces are TASK-009/010.

### 12.4 Synthesis models (Decided: design-music.md section 4)

Every Decided model — sampler, analog, FM, phase distortion,
additive, wavetable, granular — is expressible as an `inst` chain
over the extended `UGenSpec` catalog above, and each SHIPS AS A
PRELUDE TEMPLATE: the templates (`sampler`, `analog`, `fm`, `pd`,
`additive`, `wavetable`, `granular`) are prelude `.vact` SOURCE
compiled through the ordinary `inst` -> `InstDef` path at session
start — no second definition mechanism exists, so `s :analog` and a
hand-written chain are the same machinery, and every template
parameter is a pattern control like any other. Wavetables come from
the host bank or are built from a sample on the evaluator/IO side
(`TableRef` into preallocated table storage, same 16.1 lifecycle as
samples). Voice cost varies by model; the voice pool stores per-node
state sized at graph-swap time per template (never in the callback),
and per-model polyphony caps come from the host tier (12.7).

### 12.5 Effects, buses, and analyzers (Decided: design-music.md section 5)

```rust
pub struct EffectSpec { kind: EffectKind, params: Box<[(CtlId, Ctl)]> }
pub enum EffectKind { /* the Decided builtin catalog, grouped as the spec
    tables it: dynamics, eq-filters, delay, reverb (incl. Convolution(IrRef)),
    saturation, modulation, lo-fi (Codec/Radio take a keyword kind), resonator,
    spatial, restoration, Granulate (12.6), utility, Analyzer(AnalyzerKind) */ }
pub struct BusDef { id: BusId, chain: Box<[EffectSpec]> }   // `bus :name:` and `master`
pub struct BusGraph { buses: Box<[BusUnit]>, master: BusUnit }
```

One catalog, three positions, one representation:

- **Pattern controls** (`> room 0.3`, SuperDirt style): the control
  router maps the effect-named control onto the corresponding unit's
  parameter on the slot's bus (per-event sends), exactly as `gain`
  maps onto `amp`.
- **Inst ugens**: `UGenSpec::Effect(EffectSpec)` inside a chain.
- **Bus chains**: `bus :name:` declares a `BusDef` compiled like an
  `InstDef` (evaluator thread, dry-validated); a slot routes into it
  via the `bus` control; `master` is the root bus every bus feeds.
  Bus chains swap with the SAME generation + refcount retirement
  lifecycle as instrument graphs (12.2 native, 16.1 browser); bus
  units are preallocated at install, parameters smoothed, and
  convolution IRs are resources with the sample lifecycle
  (native-tier lengths per 12.7).

**Analyzers** (`level`, `spectrum`, `spectrogram`, `oscilloscope`,
`pitch-meter`, `stereo-meter`, …) are `EffectKind::Analyzer` units:
audio-transparent taps that WRITE preallocated `f32` analysis cells
(ring-buffered for spectrogram/oscilloscope frames) and never alter
the signal. The cells feed `Sig::Analyzer` (10.5) and the editor's
meters and scopes over telemetry (14.4) — the Decided "visual
feedback, never audible". Effects are ordinary functions, so a
PACKAGE (5.7) defines new effects as chains of these builtins —
no native extension path exists.

### 12.6 Granular (Decided: design-music.md section 6)

One engine, two faces, one implementation: `UGenSpec::Granular` (the
instrument face, granulating a sample or wavetable source) and
`EffectKind::Granulate` (the effect face on a bus or live input) both
instantiate the same grain engine.

**Sources and buffers.** A grain SOURCE is one of two storages:
- STATIC (`GranSrc::Sample(BankRef) | Table(TableRef)`): immutable
  audio in the sample arena; every position in `[0, len)` is valid
  for the resource's lifetime (12.2/16.1 retirement rules apply).
- LIVE (`GranSrc::Bus`): a per-instance preallocated circular
  CAPTURE BUFFER of `max_capture_seconds` (tier cap, 12.7),
  allocated at graph install and owned by the unit. Its single
  WRITER is the unit's own `process`, appending each incoming bus
  block at the write head; readers are grains. VALID READ WINDOW:
  a grain may start only where its whole span stays within
  `[write_head - capacity + guard, write_head)` for its lifetime,
  with `guard` = grain length + one block — so a live grain NEVER
  reads samples overwritten while it plays; a spawn whose span
  would violate this is SKIPPED and counted (diagnostic on
  sustained skipping, never a stale read, never an allocation).

**Freeze.** `freeze` (a control, applied at block boundaries) stops
the WRITE head: capture halts and the buffer retains what it holds;
grains — running and newly spawned — keep reading the FROZEN
content (old audio; nothing is overwritten because nothing is
written), and the CAPTURED EXTENT becomes the valid read window
while frozen. **Captured extent (partial fill).** The buffer
tracks `filled` (samples written since install, saturating at
`capacity`); at ALL times — live, frozen, and across the
transition — the valid window is intersected with the captured
extent, so freezing before the buffer has filled exposes only
recorded audio: grains beyond `filled` are skipped and counted,
and freezing immediately after install (nothing captured) spawns
no grains at all — uncaptured storage is never promoted to audio.
**Unfreeze transition (active frozen grains are protected;
bounded, allocation-free).** Unfreeze resumes writing at the
frozen head, which would overwrite the oldest region first — the
region frozen-admitted grains may still be reading. At the
unfreeze block boundary, every ACTIVE grain is re-checked against
the LIVE validity rule for its REMAINING span (span within
`[head - capacity + guard, head)` for its remaining life): a
grain that passes continues untouched; a grain that fails is
SHORT-GATED (the same few-ms bounded fade as voice gating, no
allocation) BEFORE capture resumes, so no sounding grain ever
reads a sample overwritten during its life — the no-overwrite
invariant holds across the transition, closing Astra's
counterexample (a long frozen grain reading the oldest region is
gated at unfreeze rather than having its source rewritten
underneath it). Grains spawned after unfreeze are admitted under
the live rule as usual. Freeze on a STATIC source pins
`position` (stops position following) — same control, source-
appropriate meaning, both stated.

**Scheduling.** Each instance owns a phase ACCUMULATOR advanced per
block: inter-onset interval = 1/`density` seconds (control-rate
read); when the accumulator crosses an onset, a grain spawns from
the preallocated pool with position (+ `spray` jitter), pitch
ratio, duration = `size`, envelope (`:hann`, `:tri`, `:trapezoid`,
`:expo` — table lookup), and `reverse` decided by the seeded
per-instance RNG stream — onset count and timing are DETERMINISTIC
given controls, seed, and block layout. Grain pool = ceil(
`max_grain_density` x `max_grain_size`) slots (BOTH tier caps,
12.7), allocated at graph install, never in the callback; a spawn
finding the pool full is skipped and counted.

**Admission.** `density` above `max_grain_density` or `size` above
`max_grain_size` (and, for live sources, `size` or position depth
beyond the capture window) is a DIAGNOSTIC with origin at the
value's arrival — grains beyond the cap are not spawned; audio
continues (never a dropout, never silent truncation of the caps
themselves). Every parameter is a pattern control;
`position`/`density`/`size`/`freeze` arrive as `Ctl` values or
cells like any control (11.4).

### 12.7 Host tiers and capabilities (Decided: architecture.md Host Tiers)

```rust
pub struct CapabilitySet { max_voices: u16, max_ir_seconds: f32,
    max_grain_density: f32, max_grain_size: f32,      // grains/sec, seconds (12.6)
    max_capture_seconds: f32,                          // live granular buffer (12.6)
    multichannel: u8, offline_render: bool,
    midi_in: bool, midi_out: bool, file_access: bool }
```

`HostManifest` carries the tier's `CapabilitySet`; the browser tier
advertises the worklet-budget subset, the native tier (the same core
compiled natively inside the Tauri app, CoreAudio via `cpal`,
CoreMIDI — the second milestone) advertises richer limits. The
LANGUAGE is identical on both tiers; the checker and runtime turn any
use beyond the advertised capability (an IR too long, a density too
high, MIDI without a device, offline render on the browser tier) into
a diagnostic with origin — never a crash, never silent truncation.
Swift is glue only (session/audio-background/AUv3 later) and outside
this plan.

### 12.8 Back-end implementation decisions (TASK-007..008, 2026-09-25)

This section pins the choices that sections 10.3-10.4, 11.2-11.7, 12,
16 and 17 leave open for issue #3 (scheduler, slot table, capability
hosts, DSP graph, native and wasm hosts). It follows the shape of 6.5
and 7.1 and changes no Decided behavior. The semantics of staging, the
occurrence merge, rebind, `SlotControl`, control cells, note lifetime
and the 16.1 lifecycle stay exactly as written in 11.3, 11.7, 12.6 and
16.1; this section only fixes where they live, how they are wired and
how they are proven. Open author questions are in
`design-docs/user-qa/pending-backend-questions.md` (B1-B5); the
implementation follows each recommendation until it is answered.

#### 12.8.1 Scope boundary

In scope: every TASK-007 and TASK-008 deliverable and completion
criterion of `impl-plans/active/vactr-core.md`, with these
boundary decisions:

- **No `Session` yet.** `Session` is TASK-009. TASK-007 adds
  `sched::Runtime`, which owns the slot table, the scheduler, the
  authoritative `ControlCells`, the clock, the `InputCells`
  (7.1.3), the telemetry queue and the host bundle. TASK-009's
  `Session` wraps `Evaluator` + `Runtime` and does not re-implement
  either. Where 11.5 writes `dry_run(&Binding, &Session)`, the
  signature is `dry_run(&Binding, &mut dyn QueryVm, &InputCells,
  seed) -> QueryResult`.
- **Session socket moves to TASK-009 (B5).** The tungstenite session
  socket listed under `NativeAudioHost` needs the `ClientMsg`/
  `ServerMsg` protocol and the token rule of 17, both TASK-009. No
  TASK-008 criterion exercises it. TASK-008 adds no socket and no
  tungstenite; the core plan's TASK-008 deliverable text is amended
  by the FINAL wave and TASK-009 gains the item.
- **No real OSC transport, no RenderHost implementation, no WebMIDI.**
  `OscHost`, `RenderHost`, `MidiHost` and `MidiInHost` are traits
  with `NoopHost` and recording test hosts. The native host
  implements `MidiHost`/`MidiInHost` over midir (TASK-008). A UDP OSC
  host, the WebGL `RenderHost` and WebMIDI wiring are TASK-010's TS
  shell. The browser tier's `CapabilitySet` therefore advertises
  `midi_in = midi_out = false` until TASK-010 flips it.
- **Visual slots.** `out o0..o3` bind `Binding::Texture` in the same
  table. On activation the scheduler calls `RenderHost::set_program`;
  `stop`/`hush` call it with the empty program (black). Per-frame
  uniform evaluation (9.3) needs a render loop and is TASK-010's.
- **Spec fixtures.** The FINAL wave reclassifies the four blocks
  `deferred_to = "TASK-008"` (design-music ordinals 2, 4, 5, 6) to
  `positive` or `diagnostic` with pinned multisets, evaluated with
  the fixture `NoopHost` (so `load`/`sample` I/O still fails
  `host-unavailable` where a block needs files).

#### 12.8.2 Files and the 800-line budget

No `.rs` file may reach 800 lines (hard limit 1000). Tests go in
`tests/` submodules as in 6.5/7.1. Owner waves are in 12.8.12.

**Skeleton rule (no shared module files after CONTRACTS).** CONTRACTS
creates every module declaration and file this map names, so that no
later wave edits a `mod.rs` it does not own:
- `src/lib.rs` (`pub mod host;`), `host/mod.rs` (`pub mod native;`
  gated by `all(feature = "host-native", not(target_arch = "wasm32"))`,
  `pub mod wasm;` gated by `all(target_arch = "wasm32", feature =
  "host-wasm")`), `sched/mod.rs`, `dsp/mod.rs` (including `pub mod
  build;`), `ns/mod.rs` (`insts`), `vm/natives/mod.rs` (`dsp`), each
  with every entry of the table;
- a stub (a doc comment only, and for `examples/beep.rs` an empty
  `fn main`) for every file a later wave owns, including the
  subdirectory roots `dsp/ugen/mod.rs`, `dsp/effects/mod.rs`,
  `host/native/mod.rs` and `host/wasm/mod.rs`;
- the test skeletons: `<module>/tests/mod.rs` declaring one file per
  owning wave (`sched/tests/{sched,midi}.rs`, `dsp/tests/dsp.rs`,
  `host/tests/e2e.rs`, plus the `tests/` directories of the language
  modules INST edits, `inst.rs` in each), each created as a stub.

From then on each wave writes only its own files. A wave that needs
another file creates it inside a subdirectory whose `mod.rs` it owns
(DSP: `dsp/ugen/`, `dsp/effects/`; NATIVE: `host/native/`; WASM:
`host/wasm/`). A test file may declare its own nested modules. After
CONTRACTS, INST is the only writer of `value/`, `types/`, `compile/`,
`vm/` and `ns/` (FINAL touches only fixtures there), so the INST plan
lists the existing files it edits in those directories, native
registration in `vm/natives/mod.rs` included, as its `writePaths`.

**Enum shapes land in CONTRACTS.** CONTRACTS also declares
`Value::UGen(Rc<UGenNode>)` (`UGenNode` as an opaque shell in
`dsp/graph.rs`), `Sound::Inst(InstId)` and `Sound::Osc(Rc<str>)`, with
the minimal arms that keep existing exhaustive matches compiling
(print `<ugen>`, `<inst>`, `<osc>`; pointer equality for `UGen`;
`kind_name`). SCHED and DSP then compile against the final shapes,
and INST owns their semantics. `Ty::UGen` stays with INST, because
only the checker matches on `Ty`.

| Module | Files (owner wave) |
|--------|--------------------|
| `host/` | `mod.rs`, `caps.rs` (all capability traits, `HostSigs`, `GraphHandle`), `wire.rs` (POD priority-channel records and their little-endian byte codec, 12.8.5), `noop.rs` (`NoopHost` for every trait including `SourceLoader`; `ns::load::NoopHost` becomes a re-export), `testing.rs` (`#[cfg(test)]` recording hosts and mock transports) (CONTRACTS); `native/{mod,audio,midi,tick,loader}.rs` (NATIVE); `wasm/{mod,abi,main_half,worklet_half}.rs` (WASM); `tests/e2e.rs` and modules it declares under `tests/e2e/` (FINAL) |
| `sched/` | `slots.rs` (`SlotTable`, `Slot`, `SlotKind`), `runtime.rs` (`Runtime`, command queue, tick driver), `staging.rs` (occurrence records, coverage extension, scoped invalidation), `ledger.rs`, `commit.rs` (POD conversion, control mapping, `Const` downgrade), `control.rs` (two-class channel, monotone merge, re-send), `cells.rs` (authoritative `ControlCells`, epochs, batches), `dryrun.rs`, `telemetry.rs`, `oneshot.rs` (`once`/`at`) (SCHED); `midi_in.rs` (`cc` cells, `midi-notes`, `NoteInstance`), `midi_clock.rs` (slave, master, transport) (MIDI) |
| `dsp/` | `graph.rs` (`InstId`, `InstDef`, `UGenSpec`, `Edge`, `EffectKind`, `EffectSpec`, `BusDef`, ids), `controls.rs` (the control table, 12.8.7), `cells.rs` (`CellRead`, native `AtomicCells`, the browser `Mirror` state machine of 11.3), `release.rs` (`VoiceRelease` tag map + tombstone ring), `caps.rs` (`CapabilitySet` + tier presets), `alloc_probe.rs` (`#[cfg(test)]`) (CONTRACTS); `engine.rs` (the callback core), `voice.rs`, `ring.rs`, `ugen/{osc,filter,env,fm,additive,wavetable,sample}.rs`, `effects/{mod,dynamics,eq,delay,reverb,saturation,modulation,lofi,resonator,spatial,restoration,utility,analyzer}.rs`, `fft.rs`, `granular.rs`, `bus.rs`, `arena.rs` (`SampleArena`, slice install, credit, refcounted retirement), `meta.rs` (`EditorDecl`/`ParamMeta` table) (DSP); `build.rs` (ugen node tree -> `InstDef`/`BusDef` lowering, node cap) (INST) |
| language | `value/value.rs`, `value/print.rs`, `value/eq.rs`, `vm/call.rs`: the `Value::UGen`, `Sound::Inst`, `Sound::Osc` declarations and their minimal arms (CONTRACTS, skeleton rule above; later INST edits to these files for semantics are allowed because no other wave writes them). Then (INST): `types/ty.rs` (`Ty::UGen`), `types/natives.rs` rows, `types/check.rs`/`infer*.rs` (inst-body rules), `compile/` (implicit control names), `vm/natives/dsp.rs` (ugen, effect, bus, `osc` natives), `ns/insts.rs` (instrument registry, `InstResolver` impl), `src/prelude/templates.vact` (INST) |
| other | `Cargo.toml`, `src/lib.rs` (`pub mod host;`, test-only global allocator) (CONTRACTS); `examples/beep.rs` (NATIVE); `editor/worklet/*.js`, `editor/dev-harness/*` (WASM) |

#### 12.8.3 Runtime wiring and contracts between waves

- **Effect intake.** `Runtime::new(hosts, caps)` returns the runtime
  and a `RuntimeSink: EffectSink` that shares an
  `Rc<RefCell<CommandQueue>>` with it. The caller builds the
  `Evaluator` with that sink (7.1.3 "Staged effects"). After every
  `eval_str`/`eval_form`/`run_pass`, the caller calls
  `Runtime::drain(&mut Evaluator)`, which applies queued commands with
  VM access (`Evaluator::vm_and_ns`): a `SlotBind` runs the dry run
  (11.5) and, on success, sets `pending`; on failure it returns the
  diagnostics and the old binding keeps playing. `Evaluator` needs no
  change beyond this.
- **Tick.** `Runtime::tick(&mut Evaluator, host_now: f64)` runs the
  six steps of 11.3 against a `VmQuery` (Query effect mode) and the
  runtime's `InputCells`, then runs due `at`/`once` thunks in Normal
  mode through the evaluator. Returned `TickReport` carries
  diagnostics, forwarded console lines and telemetry counts.
- **Instrument resolution** (the one cross-wave trait, defined by
  CONTRACTS in `host/caps.rs`): `trait InstResolver { fn route(&self,
  sound: &Sound) -> Result<Route, Failure>; }` with `Route = Audio
  { inst: InstId } | Midi { ch: u8 } | Osc { addr: Rc<str> }`. SCHED
  tests use a stub; INST implements it over the instrument registry
  (12.8.6). Commit calls it once per emitted event.
- **Per-event sink routing (supersedes 11.2 `SlotKind::{Audio, Midi,
  Osc}`).** Decided 2026-09-25: MIDI and OSC are instruments selected
  by `s`, and d1..d9 are the only sinks. A slot therefore has
  `SlotKind = Pattern | Texture`; each committed event goes to the
  `AudioHost`, `MidiHost` or `OscHost` its `Route` names, and every
  `SlotControl` for a pattern slot goes to all three sinks, keeping
  one (slot, gen) identity across them (11.4).
- **Control writes.** A `CellUpdate`, `TweakRefresh` or `Bindings`
  command writes the authoritative cells and invalidates uncommitted
  staging of every slot from the commit horizon on (11.3 "Control
  writes and STRUCTURE"). Scoped invalidation `invalidate(slot, span)`
  is an internal API the tests call directly.
- **Captured output.** Query output is attached to the staged query
  fragment that produced it. It is forwarded when the first record of
  that fragment commits, or when the fragment's end passes the commit
  horizon with no record; invalidating the fragment discards it, and
  restaging captures afresh (11.3 step 4, 10.4).
- **Input cells.** `Runtime` owns `InputCells`; `cc` draining (MIDI
  wave) and analyzer/host-signal cells published by the audio side
  write it once per tick.

#### 12.8.4 Time, horizons and thresholds

- `host_now` is `f64` seconds on the AUDIO timebase of the host:
  native = frames rendered / sample rate from an atomic counter the
  callback advances; browser = the worklet's posted `currentFrame /
  sampleRate` (16). Tests use a mock clock.
- Defaults (all `Runtime` config fields): `lookahead` 120 ms,
  `commit_lead` 30 ms, control re-send after 3 unacked ticks,
  host-transport diagnostic after 20 unacked ticks, MIDI clock loss
  timeout 500 ms, clock smoothing factor 0.1, latency auto-widen by
  10 ms (cap 200 ms) when more than 4 late events arrive within one
  second, reported as `latency-widened`.
- Seconds conversion happens once per event in `commit.rs` through
  `Clock` (11.1); nothing else in `sched/` holds `f64` time.

#### 12.8.5 Channels and wire records

Both tiers carry the same message set; only the transport differs.

- **Evaluator -> audio**: the time-ordered event ring (`AudioEvent`,
  11.4) and ONE priority control channel per sink carrying `CtlMsg =
  SlotControl | CellInit | CellBatch | CellRetire | LiveNoteOn |
  VoiceRelease | GraphInstall | GraphRetire | SampleSlice |
  SampleRetire`.
- **Audio -> evaluator**: `HostMsg = SlotControlAck | CellInitAck |
  CellBatchAck | Retired | SliceOk | Installed | Counters (late,
  dropped, stolen, skipped) | AnalysisCells`.
- **Native**: the ring and each channel are preallocated lock-free
  SPSC rings of POD records; cells are `AtomicCells`
  (`AtomicU32` bit patterns, release store / acquire load), so
  `CellInit`/`CellBatch` are not sent natively. Graph swaps use the
  12.2 triple buffer with the retired structure dropped on the
  evaluator thread.
- **Browser**: `host/wire.rs` encodes every record as fixed-layout
  little-endian bytes. JS never parses a record; it moves
  `ArrayBuffer`s between the two instances (12.8.10).
- **Mock transports** (`host/testing.rs`): a native model (shared
  `AtomicCells`, immediate) and a browser model (an isolated `Mirror`
  behind a FIFO with configurable delay, loss and stall, no
  shared-memory shortcut), used by the 11.3 cell and control tests.
  The `Mirror` is the same `dsp/cells.rs` code the worklet runs.

#### 12.8.6 Instruments, ugens and buses

- **Value and type.** `inst` bodies build a node tree: `Value::UGen(Rc
  <UGenNode>)` with checker type `Ty::UGen`. A ugen input accepts
  `float`, `int`, `ugen` or `signal`; the unifier allows those only at
  ugen input positions (the same local-coercion shape as `[sound]` in
  7.1.4). `+ - *` get a `ugen` overload (a node-building native when
  either operand is a ugen).
- **Realization.** `inst` still compiles to a closure (7.1.4
  "Definition heads"). At definition time INST calls it ONCE with
  each header parameter bound to a `Param(CtlId)` node (M3: header
  parameters are control names), lowers the returned tree to an
  `InstDef` in `dsp/build.rs`, registers it, and installs it on the
  audio side. A header default that is a `Direct` tweak site becomes
  a cell-backed `Ctl::Cell` default (11.3); other defaults are
  `Ctl::Const`. More than 256 nodes is `graph-too-large` (16.1). A
  failing body is `Failure(inst-failed)` for the defining form and
  leaves any previous definition installed.
- **Implicit control names (B2).** Inside an `inst` or `bus` body, a
  free name that is a row of the control table (12.8.7) and is not
  bound in scope compiles to a `Param(CtlId)` node, typed from the
  row. This is how `attack`, `release`, `amp`, `note` and `n` in the
  design-music examples resolve without header declarations.
  Everywhere else such a name is still `undefined-name`.
- **Name collisions (extends the M1 fixed set, B2).** `saw`, `tri`,
  `lpf`, `hpf`, `bpf`, `delay`, `comb`, `gain`, `pan`, `room`, `bus`,
  `range` and every effect name that is also a pattern control join
  the subject-overload group: a `pattern` subject selects the existing
  pattern control or signal; a `ugen` subject, a numeric argument
  inside an `inst` body, or no subject inside a `bus` body selects the
  ugen/effect. Zero-argument `saw`/`tri` stay the signals.
- **Signals as ugen inputs** (`shape: {range sine 0 1}`) become
  control-rate cells (12.2 "control cells"): the scheduler samples the
  signal once per tick for each installed instrument that uses it and
  writes the cell; the voice reads it continuously. No closure runs on
  the audio side.
- **Sound resolution.** `inst NAME` binds `NAME` to
  `Value::Sound(Sound::Inst(id))` and registers `:NAME`. `s :k`
  resolves `k` in the kit first (7.1.4), then in the instrument
  registry (prelude templates and session insts), else
  `Failure(unknown-sound)`. `Sound::Builtin(k)` for a host sample
  bank routes to the `sampler` template with `bank = k` ("every sample
  IS a sampler"). `osc "/addr"` returns `Sound::Osc`. The checker's
  known-key set adds the template names (the host manifest's synth
  set) and the document's `inst` names, as 7.1.4 already says.
- **Templates.** `sampler`, `analog`, `fm`, `pd`, `additive`,
  `wavetable`, `granular` are ordinary `inst` definitions in
  `src/prelude/templates.vact` (embedded with `include_str!`),
  evaluated into the prelude at runtime construction through the
  normal path. They use the plain header form (`inst analog wave: ...
  :`), not the `inst drum: sampler ...:` spelling of design-music
  section 4, whose meaning is open (B3).
- **Buses.** `bus :name:` + block and `master:` + block are definition
  heads like `inst`; the block's first element takes the bus input as
  its implicit subject. They lower to `BusDef` and swap under the
  generation + refcount lifecycle (12.5). `bus :name` with a pattern
  subject is the routing control.

#### 12.8.7 Control table

`dsp/controls.rs` holds one row per control name: `CtlId`, default,
range, and route = `InstParam` (by name, including `freq` and `amp`),
`OrbitFx(unit, param)`, `BusUnit(param)` (`room` maps to the bus
reverb unit's parameter) or `Scheduler` (`orbit`, `bus`, `cut`,
`legato`). `note`/`n` map to `freq` through the scale, and `gain` maps
to `amp` (Q4). `n` on a bank selects the sample (7.1.4). `Ctl` carries
only `f32`, so commit encodes a keyword control as its index in the
row's enum list (`wave`, `envelope`, `kind`), a bank or table keyword
as the installed resource id, and a boolean as 0 or 1; a value outside
the row's domain is an event-local `Failure(type)`. List-valued
header arguments (`partials`) are realization-time constants of the
node, not controls. A control an
instrument does not declare is ignored (SuperDirt behavior), with no
diagnostic. `MAX_CTLS = 24`; an event with more resolved controls is
an event-local `Failure(too-many-controls)`.

#### 12.8.8 Effect catalog and metadata

- Every design-music section 5 name has an `EffectKind` variant, an
  implementation, and an `EditorDecl`/`ParamMeta` row. One test
  asserts that each catalog name, ugen and template has exactly one
  meta row and one implementation.
- **Fidelity (B4).** Each kind is a bounded, allocation-free algorithm
  built from shared primitives (biquad, one-pole, delay line, allpass/
  FDN, envelope follower, waveshaper, LFO, fixed-size radix-2 FFT).
  Kinds whose reference-quality algorithm needs large state or
  lookahead (`linear-phase-eq`, `group-delay-eq`, `pitch-shift-hq`,
  `denoise`, `codec`, `spatial-map`, `crosstalk-cancel`,
  `fir-crossover`) ship a documented approximation with fixed
  preallocated state. The acceptance bar for every kind is finite,
  bounded output, correct bypass (`mix 0` or `section` off is
  bit-identical to the input where the kind has a mix) and zero
  callback allocation. Sound quality is not an acceptance criterion
  in v1.
- Analyzers are transparent taps writing preallocated `f32` cells;
  the bit-compare test renders with and without the analyzer.
- **Capability checks.** `CapabilitySet::require(cap, origin) ->
  Result<(), Diagnostic>` is the one gate. INST calls it at
  realization or install (IR length against `max_ir_seconds`, grain
  caps, voice counts), and the scheduler calls it at commit for values
  that arrive at run time (12.6 admission). Offline render has no
  language verb in v1, so its criterion calls `require(OfflineRender,
  origin)` on the browser preset and asserts the "not available on
  this host" diagnostic.

#### 12.8.9 Audio engine and the allocation proof

- `dsp::Engine::process(&mut [f32], frames)` is the one callback core
  both hosts call. It drains the priority channel first, then the
  ring (events due in the block, sample-accurate start at
  `round((time - block_start) * sr)`, late events start at frame 0
  and are counted), renders voices, orbit effects and buses, and
  publishes counters. It is generic over `CellRead` (native
  `AtomicCells`, browser `Mirror`).
- Capacities are fixed at construction from `CapabilitySet`: voices
  (browser 64, native 256), ring 1024 events, channel 256 records per
  sink, tag map = voice count, tombstone ring 64, grain pools per
  12.6. Short gate = 3 ms linear fade.
- **Allocation probe.** `dsp/alloc_probe.rs` defines a counting
  global allocator installed in `src/lib.rs` under `#[cfg(test)]`
  only, so release, example and wasm builds never contain it. It
  counts `alloc`/`realloc` on the current thread while a thread-local
  flag is armed (`const`-initialized, so arming allocates nothing).
  Headless render tests arm it around every `Engine::process` call
  and assert a zero count. This is the "debug-mode counters" proof;
  the tests are unit tests inside the crate because an integration
  test would not see `cfg(test)`.

#### 12.8.10 Hosts

- **Cargo.** `rust-version = "1.83"`. `host-native = ["dep:cpal",
  "dep:midir"]` with both crates optional under
  `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`, and
  `host/native` compiled only under `all(feature = "host-native",
  not(target_arch = "wasm32"))`, so both wasm32 builds stay green.
  `host-wasm` pulls NO crates (next point). `[lib] crate-type =
  ["rlib", "cdylib"]` so the wasm32 build emits `vactr.wasm`.
  Version policy: the highest cpal/midir releases whose
  `rust-version` and whole resolved tree build on 1.83 (cargo 1.83
  has no MSRV-aware resolver, so transitive crates are pinned with
  `cargo update --precise` where needed); prefer cpal 0.16.x (no
  bindgen on macOS), fall back to 0.15.3. The chosen versions,
  `Cargo.lock` diff and a `cargo audit` run are recorded in the
  CONTRACTS evidence.
- **NativeAudioHost** (`host/native`): a cpal output stream whose
  callback runs `Engine::process`; `now()` from the frames counter;
  a timer thread posting tick wakes over `std::sync::mpsc` every 5 ms
  to the evaluator thread; midir input (first port or a configured
  name) feeding `MidiInHost`, midir output for `MidiHost`; no device
  = `beyond-capability` ("not available on this host"). The native
  `SampleLoader` reads WAV only (RIFF PCM 16/24/32-bit int and 32-bit
  float, mono or stereo) with an in-house parser, from configured
  directories (17); a file sample rate other than the engine's scales
  playback rate. Other formats are `Failure(host-unavailable)`.
- **examples/beep.rs** (`required-features = ["host-native"]`):
  builds `Evaluator` + `Runtime` + `NativeAudioHost`, evaluates
  `s :analog > note [:a4] > once` (or `s {sample PATH} > once` when a
  WAV path is given), ticks for two seconds and exits. Its audible
  check is manual and pending user confirmation; the automated proxy
  is the FINAL headless render of the same program.
- **WasmHost, no wasm-bindgen (divergence from section 4 and the core
  plan text).** wasm-bindgen needs its CLI to generate JS glue, which
  is a build step beyond the wasm module. The module instead exports
  a raw `extern "C"` ABI under `all(target_arch = "wasm32", feature =
  "host-wasm")`: `alloc`/`free` for JS-written input bytes; main half
  `main_init`, `eval(ptr, len)`, `tick(now)`, `inbox(ptr, len)` (a
  `HostMsg` from the worklet) and `outbox_ptr`/`outbox_len`/
  `outbox_clear`; worklet half `worklet_init(sample_rate,
  capacities)`, `worklet_inbox(ptr, len)`, `process(frames) ->
  out_ptr` and `report_ptr`/`report_len` (counters for the harness).
  The core needs no JS imports.
- **Worklet glue** (`editor/worklet/processor.js`, `host.js`, plain
  JS): the main thread fetches the module bytes, instantiates wasm #1,
  and posts a copy of the bytes to the `AudioWorkletProcessor`, which
  instantiates wasm #2 during init and renders silence until ready.
  The worklet posts its frame time every render quantum; the main
  thread calls `tick`, then posts each outbox record as a transferred
  `ArrayBuffer`. The worklet `onmessage` handler only stores the
  buffer reference in a preallocated fixed-size JS slot array
  (overflow = drop + count), which is O(1). `process()` drains the
  slots into `worklet_inbox`, where all copying happens under the
  16.1 per-quantum credit (at most `INSTALL_BYTES_PER_QUANTUM` of
  slice copy and one `CellBatch` per call), then renders. Browser
  sample decode is `decodeAudioData` on the main thread (off the
  audio thread, 16.1).

#### 12.8.11 Dev harness and real-worklet evidence

- `editor/dev-harness/index.html` + `harness.js` load the wasm32
  host-wasm build and the worklet glue, run the checks of the two
  TASK-008 real-worklet criteria (cell transport and `VoiceRelease`;
  the 16.1 lifecycle list) with synthetic sample buffers, and read
  results from the worklet's report counters. They also assert that
  worklet wasm memory size is unchanged from the end of init to the
  end of the run (no growth, 16.1).
- **Automated run (primary).** `node editor/dev-harness/
  run-headless.mjs` (node built-ins only: `http`, `child_process`,
  `fs`) serves the harness on `127.0.0.1` (a secure context for
  worklets), launches Chrome from `$CHROME` or the default macOS path
  with `--headless=new --autoplay-policy=no-user-gesture-required
  --user-data-dir=<tmp>`, receives the page's JSON report by POST,
  writes it to `target/fe-logs/be-wasm-harness-s<session>-<n>.json`,
  and exits 0 when every check passes, 1 on a failed check or a
  missing `target/wasm32-unknown-unknown/debug/vactr.wasm` (built
  by the `wasm32-hostwasm` check first), and 2 when blocked (Chrome
  missing, or the realtime `AudioContext` never reaches `running`),
  with a 120 s timeout.
- **Blocked is not passing.** If the workflow sandbox cannot run
  Chrome or open an audio context, the run is recorded as blocked,
  the operator runs the same page headed (`run-headless.mjs --headed`)
  and the resulting report is the evidence. That goes beyond the
  issue's single beep exemption, so it is surfaced at review (B1),
  not silently accepted. The headless `Engine::process` tests never
  substitute for these checks (16.1).

#### 12.8.12 Codes, waves and verification

**New codes** (closed list). `DiagCode`: `graph-too-large`,
`host-transport` (unacked controls past the threshold),
`ring-overflow`, `latency-widened` (w), `arena-exhausted`,
`install-queue-overflow`, `grain-skip` (w, sustained skipping),
`voice-steal` (w), `clock-lost` (w), `clock-external` (`use-bpm` under
`:midi`). Capability misses reuse `beyond-capability` with the message
"not available on this host". `FailCode`: `inst-failed`,
`too-many-controls`. CONTRACTS adds them all in one edit, as in 7.1.3.

**Waves.** Plans `impl-plans/active/vactr-backend-<wave>.md`; each
lists its own plan file in its manifest `writePaths`, and only FINAL
edits `vactr-core.md`, `impl-plans/README.md` and the spec fixture
manifest (it never edits the dispatch manifest).

| Wave | Content | Depends on |
|------|---------|------------|
| CONTRACTS | Cargo features/deps (including the `[[example]] beep` entry) and version pinning, the 12.8.2 skeleton (every `mod.rs`, stubs, test skeletons), the enum shapes of 12.8.2, `host/{caps,wire,noop,testing}.rs`, `dsp/{graph,controls,cells,release,caps,alloc_probe}.rs`, the codes (`SlotId`/`CtlId` stay where they are in `sched/slots.rs`, which SCHED then owns) | — |
| SCHED | TASK-007 core: slot table, staging, occurrence merge, ledger, commit, control channel, cells, dry run, telemetry, `once`/`at`, tempo change, captured output | CONTRACTS |
| DSP | TASK-008 pure DSP: engine, voices, ring, ugens, effects, FFT, granular, buses, analyzers, arena, meta | CONTRACTS |
| INST | language side: `UGenNode` contents and the semantics of the CONTRACTS enum variants, `Ty::UGen`, natives, inst realization, implicit control names, overloads, registry + `InstResolver`, `bus`/`master`, `osc`, prelude templates, `dsp/build.rs` | CONTRACTS |
| MIDI | `sched/midi_in.rs`, `sched/midi_clock.rs`, `sched/tests/midi.rs`, plus `sched/runtime.rs` (to wire the MIDI calls into `tick`/`drain`) and `clock/clock.rs` (only if the slave anchor needs a change): `cc` cells, `midi-notes` realization, note lifetime (voice termination asserted against `dsp::Engine`), clock slave/master over the existing `clock::MidiClockSync`, transport, `use-clock`, `midi-clock-out` | SCHED, DSP |
| NATIVE | `host/native/*`, `examples/beep.rs` | SCHED, DSP, INST |
| WASM | `host/wasm/*`, `editor/worklet/`, `editor/dev-harness/` incl. the headless runner and its run | SCHED, DSP, INST |
| FINAL | `host/tests/e2e.rs` + `host/tests/e2e/` (headless end-to-end: sample-region handoff through partitioned staging to the ring, template renders from prelude source, bus chains, the beep proxy), fixture reclassification, core plan checkboxes and progress log, README, archive | MIDI, NATIVE, WASM |

Waves run as CONTRACTS; SCHED + DSP + INST; MIDI + NATIVE + WASM;
FINAL.

**MIDI wiring (chosen: MIDI owns the `runtime.rs` edit).** SCHED
leaves `Runtime` with the MIDI settings recorded but not acted on: the
`use-clock` and `midi-clock-out` staged effects are stored (`:link`
already fails in the checker, 7.1.6), and the `MidiInHost` in the host
bundle is never polled. In wave 3, MIDI adds the calls at the tick and
drain points of `sched/runtime.rs`:
- drain `MidiInHost` into `cc` cells and live `midi-notes` events;
- close `NoteInstance` records on `stop`/`hush`;
- apply the clock slave anchor, Start/Stop/Continue freeze and resume,
  and clock-loss freewheel;
- emit clock-master output.

MIDI is the only wave-3 writer of `sched/runtime.rs` and `clock/`.
NATIVE and WASM write only `host/native/`, `host/wasm/`,
`examples/beep.rs` and `editor/`, and depend only on the
`MidiInHost`/`MidiHost` traits, so the three wave-3 write sets are
disjoint. No hook trait is added, because only one caller exists.

**Verification.** The 6.5.7 evidence rule applies to every wave with
`<plan>` = `be-<wave>`. The checks are `build`, `clippy`
(`--all-targets -- -D warnings`), `fmt` (`--check`), `nextest`, `test`
(plain `CARGO_TERM_QUIET=true cargo test`), `wasm32`,
`wasm32-hostwasm` and `linecount` (largest `.rs` under 800); CONTRACTS
adds `audit`, WASM adds `harness` (12.8.11), NATIVE adds
`cargo build --example beep`. Rollback is `git revert` of the single
implementation commit; nothing migrates.

## 13. Controller Binding (editor/runtime affordance, never syntax)

The published code contains only music plus, in Directive mode,
comments the language ignores — no keyword, no annotation the
language reads (13.5). Two bindable site kinds, both riding the
late-bound var mechanism:

1. **Top-level `let`/`var` bindings.** Binding a controller to `cutoff`
   makes the editor issue `set-var` protocol messages. For a `var` the
   session applies `upd`: LATE readers hear it per the 11.3 cell
   contract — within the commit horizon natively, plus one control
   hop in the browser (delivery-dependent, stated in the editor's
   latency tier) — and EAGER dependents (derived bindings computed
   from the var) recompute through the reactive pass of 5.6, landing
   at the boundary swap with the display batch updated. For a
   `let` the session performs live redefinition PLUS dependency-aware
   re-evaluation: the reactive dependency graph (5.6) re-evaluates the
   dependent top-level forms through dry run and the normal swap
   discipline, so patterns that snapshotted the `let` are rebuilt —
   exactly what re-typing those lines does. The effect lands at the
   next cycle boundary, not the next event, and the editor displays
   that latency tier.

2. **Numeric literals in top-level forms** (a gain in a chain, a step
   value, an `inst` argument at a call site). The compiler assigns each
   such literal a stable `TweakId` (hash of file, enclosing top-level
   form path, and literal index) and compiles it as `LoadTweak t`
   instead of `LoadConst`: an anonymous `VarSlot` of kind `Tweak`,
   initialized to the literal's value, late-bound like any var. Pattern
   and inst constructors therefore hold a ref and hear changes at the
   next event with no re-evaluation of the form.

```rust
pub struct TweakSite { id: TweakId, span: Span, slot: Rc<VarSlot>,
                       initial: Value, ty: NumTy, tier: SiteTier,
                       form_gen: FormGen }
pub enum SiteTier { Direct, Reeval, Manual }
```

**Site tiers.** Not every literal can stay a live reference: arithmetic
forces and dereferences (5.5, 5.6), so a literal that flows through
computation (`gain {* 0.5 2}`) is consumed when the form is evaluated.
The session therefore classifies every site honestly and tells the
editor which mechanism serves it:

- `direct` — the literal or `var` is captured as `PParam::Late` /
  `VParam::Late` by a pattern, control, signal, or `inst` argument.
  `set-tweak` writes the slot with no re-evaluation; audibility
  follows the 11.3 per-tier cell contract: within the commit
  horizon natively, plus one control hop in the browser
  (delivery-dependent — never asserted unconditionally).
  Probabilities and all numeric combinator parameters
  are `PParam`s (10.1), so `maybe 0.3` is a `direct` site.
- `reeval` — the value flowed through computation (arithmetic, a `let`,
  a function call). `set-tweak` updates the tweak slot AND triggers
  the automatic rebuild of the owning top-level form under the
  eligibility, replayability, atomicity, and cycle rules of 5.6,
  landing at the swap discipline (next cycle boundary).
- `manual` — the owning form is non-replayable (one-shot effects) or
  no longer a current owner. The tweak slot still updates, but the
  change is heard only when the user re-evaluates the form
  deliberately; the editor displays this state instead of pretending
  liveness.

Protocol (section 14): the session sends the tweak-site table —
`{ id, span, tier: direct | reeval | manual, value, form_gen }`,
tagged with the source `doc_revision` — after each eval; the editor renders affordances (drag
a literal, MIDI-learn, on-screen slider); controller input becomes
`set-tweak { id, form_gen, value }` messages applied on the evaluator
thread — rate-limited by the editor, coalesced by the session to
latest-wins per tick. A `set-tweak` whose `form_gen` is stale (the form
was re-evaluated or edited since) is REJECTED with a `stale-binding`
notice, so a delayed message can never target the wrong site. Binding
identity lives EDITOR-side, keyed to text positions the editor maps
across edits with its change history; when a fresh site table arrives,
bindings re-key to the new `TweakId`s by mapped position, and sites
that no longer exist show as unbound. **Write-back**: committing a
tweaked value into the source is a text edit the editor performs only
after verifying the current text at the mapped span still spells the
site's last-known literal; on mismatch it declines and notifies.
**Persistence is MODE-SCOPED per 13.5 — the earlier unconditional
"never in the file" claim is superseded.** In DIRECTIVE mode (the
adjudicated default) panel membership and CC mappings persist IN the
`.vact` file as `#@` comments, learned CCs are written back into
them, and saving retains them — comments only, never a language
construct. In EXTERNALFILE mode the saved source contains no binding
data and mappings live in an editor-side session file. In BOTH
modes, overlay tweak VALUES never reach the source without an
explicit commit, and ranges/curves/editor kinds are never persisted
anywhere (they come from `ParamMeta`).
Frozen mode compiles literals as constants; tweaks are a Live mode
feature, in the runtime, not the language — consistent with the "no
core-language feature may make Frozen mode impossible" rule.

**Site scope — exactly the Decided enumeration.** The editor
enumerates every numeric site from the checker's AST, per
architecture.md Editor Requirements, and that Decided list is the v1
scope: (a) numeric literals inside patterns and controls in top-level
statements; (b) top-level `let`/`var` numbers; and (c) `inst`
PARAMETER DEFAULTS — a default in a definition HEADER is one
unambiguous slot per definition (it applies wherever the default is
used), so the earlier blanket definition-body exclusion is narrowed:
header defaults ARE sites. An inst-default site is `direct`-tier: the
default lives in the `InstDef` param table as a `Ctl::Cell` (11.4),
so turning its slider is heard, with no re-evaluation, at the next
voice that uses the default under the 11.3 per-tier cell contract
(natively the very next voice; in the browser after the update
batch applies); re-evaluating the `inst` migrates the
override like any site. Numeric literals inside function BODIES
remain out of scope (not in the Decided list; one body serves many
calls). The tweak-site table marks each site's origin
(`pattern-literal | binding | inst-default`) so the panel can group
them.

**Two slider modes, both Decided.** A slider (or mouse drag, or
MIDI-learned CC — three front ends to the same messages) flows back
in either of two modes the performer chooses per slider: SOURCE-EDIT
mode, where the editor performs a validated text edit of the literal
plus live redefinition of the form (the write-back path — the code
shows the moving number); or OVERLAY mode, where `set-tweak` writes
the runtime tweak slot keyed to the site's provenance and the TEXT IS
UNTOUCHED (the editor renders the overlay value beside the literal).
Publishing the file in overlay mode publishes the original literals;
"commit" folds an overlay into the source explicitly. Nothing about
either mode appears in the language.

**Override migration across generations.** Re-evaluating a form
(manual or automatic) creates fresh tweak slots. Each new site is
matched to the previous generation by its stable path within the
form, literal index, and type; a matched site whose previous slot
carried a controller override INHERITS the override — the new slot
initializes to the overridden value, not the source literal —
PROVIDED the literal's source text is unchanged. If the user edited
the literal text, the source wins and the override is dropped with a
notice. Repeated controller movement on a `reeval` site therefore
converges instead of snapping back: each rebuild inherits the latest
override, the editor re-keys to the fresh `TweakId` from the new site
table, and interim messages against the old generation are rejected
as stale and re-sent latest-wins.

Planned tests (TASK-005, TASK-009, TASK-010): a `let`-backed control
updates the sound after a boundary rebuild; a computed (`reeval`)
site converges under repeated controller movement via override
inheritance; a probabilistic `direct` site updates through restaging
(11.3); a superseded owner is not resurrected and a form that ran
`once` is excluded (reported `manual`); a failed rebuild rolls back
its overlay; a dependency cycle terminates with a diagnostic;
delayed, stale, and edit-invalidated messages are rejected;
write-back declines on text drift; duplicate literals bind
independently.

### 13.5 Directives, labels, and DAW-style parameter editors (Decided)

**Authority conflict — RESOLVED by author adjudication
(2026-09-24).** The previous revision recorded a conflict between
the workflow constraint "controller binding must not appear in the
language: no keyword, no annotation in the source" and the Decided
`#@` directive comments (architecture.md Decision Log "Directive
comments"; Editor Requirements; lang-reference.md section 1), which
persist control-panel setup and learned CC numbers in `.vact`. The
author's current workflow charter carries the adjudication forward:
the same charter that restates the constraint also ORDERS the
directive design end to end ("the SHAPE is decided by the author" —
the `#@` marker, block attachment, labels, and round-trip
write-back). The two therefore reconcile by SCOPE, per the author:
the constraint governs the LANGUAGE — no keyword, no annotation the
language reads; slots, patterns, and instruments never see a
binding — while directives are COMMENTS the language ignores as
trivia (the published code runs identically with them stripped),
and they are the Decided persistence mechanism. AUTHORITY
SNAPSHOT (re-pinned each time the authority changes; it changed
again after the previous pin, adding sample slicing and the
sampler waveform-editor requirements — implemented in 10.1 and
this section — after earlier adding the reactive graph of 5.6):
architecture.md 434 lines, sha256
`a0bf8dbc38cb4281a81369e94dd8629dda9d664bcd4af47e4d3337da2b751016`;
design-music.md 380 lines, sha256
`d90db3b7ff8bb029824d2b7c728250cf2e1dd8bd00b1a3228142c438dcb3b2fe`;
lang-reference.md 832 lines, sha256
`665ca931a464c04a8472267340c3648355e0091a3c402981c77678eafe739a07`.
`BindingPersistence` stays ONE interface: the DIRECTIVE
implementation is the adjudicated default; the EXTERNAL
session-file implementation is retained as an optional performer
preference (publish a file free of setup comments), no longer an
adjudication hedge. The binding model, sites, tiers, and provenance
are identical in both.

**Directive representation.** The reader already keeps comments with
positions (trivia); a trivia line whose comment text begins with the
fixed marker `#@` is a DIRECTIVE line. The SESSION (Rust, shared by
LSP, editor, and REPL) parses directives — the language never sees
them:

```rust
pub struct Directive { span: Span, kind: DirectiveKind,
                       attach: AttachTarget, body: DirectiveBody }
pub enum DirectiveKind { Positional, Addressed }   // by FIRST TOKEN, per the
    // Decided rule: a call-site name or label definition is Positional
    // (attaches to its line or the nearest preceding statement/block/definition
    // whose indentation is no deeper; consecutive #@ lines form one block; a
    // same-line trailing directive binds to that line); a label reference
    // (hats.hpf) or file-level setting (midi ch: 1) is Addressed and
    // position-free.
pub struct DirectiveTable { by_form: …, file_level: …, labels: LabelRegistry }
pub struct LabelRegistry;  // explicit labels (trailing `#@ name:` on the line
    // it names, `#@ name X`, block-following for bodies) + IMPLICIT labels
    // (let/fn/inst/bus/look names and slots are labels already, per the spec)
```

**Directive vocabulary (designed here per the author's
delegation; PROPOSED, not Decided).** The SHAPE (`#@`, blocks,
attachment, the positional/addressed split, labels) is Decided; the
vocabulary after `#@` was delegated to this design and remains a
proposal until the author ratifies it — tracked in the fixture
manifest's authority-question channel, never treated as Decided.
One directive per line; after `#@`:

```
directive    = file-default | label-def | positional | addressed
file-default = "midi" pair+              ; #@ midi ch: 1  (file-wide channel/device defaults)
label-def    = "name" ident              ; #@ name bass-filter  (explicit spelling; positional attach)
             | ident ":" positional?     ; trailing short form: #@ bass-filter: lpf cc: 74 71
positional   = site+ pair*               ; #@ lpf cc: 74 71  |  #@ cutoff res  |  #@ lpf hpf
addressed    = ident "." ident ["." int] pair*   ; #@ hats.hpf cc: 30 | #@ analog.cutoff cc: 1 | #@ hats.lpf.2 cc: 31
site         = a call-site name or parameter keyword, resolved against the ATTACHED statement/block
pair         = key ":" value+            ; keys: cc (ints in declared parameter order, `_` skips), ch (int)
```

- PANEL MEMBERSHIP is naming: a positional directive naming a call
  site puts that site's parameters on the control panel; naming
  parameter keywords (`#@ cutoff res` after an `inst`) selects just
  those. Ranges, curves, units, and editor kinds are NEVER written —
  they come from `ParamMeta` below, per the Decided length rules; a
  `range:` key is reserved and rejected with a diagnostic in v1.
- `cc:` numbers map positionally in the selected target's DECLARED
  parameter order; `_` skips a parameter; fewer numbers than
  parameters leaves the remainder panel-only (unmapped). `ch:`
  overrides the `#@ midi ch:` file default for that directive's
  mappings.
- ADDRESSED SELECTOR RESOLUTION (`label.sel[.n]`): `label` resolves
  through the `LabelRegistry` (explicit or implicit); `sel` then
  resolves WITHIN the label's target, in this order — (1) if the
  target is a definition with declared parameters (`inst`, `fn`,
  `bus`), a `sel` matching a declared PARAMETER keyword selects
  that one parameter (`analog.cutoff`: cutoff of inst analog);
  (2) otherwise a `sel` matching a CALL-SITE name inside the
  labeled line or block selects that call site, and the directive's
  `cc:` list maps positionally over ITS declared parameter order
  (`hats.hpf cc: 30`: the hpf call on the hats line, 30 onto its
  first declared parameter, cutoff — exactly the authority's
  example). A `sel` matching BOTH a parameter and a call-site name
  in the same target is `ambiguous-selector` (diagnosed, never
  guessed). REPEATED same-named call sites within one target (two
  `lpf` calls on a labeled line) make the bare selector
  `ambiguous-selector`; the optional ORDINAL segment is the
  concrete selectable identity — `hats.lpf.2` selects the second
  `lpf` in source order within the target (the same ordinal form
  works as a positional first token, `#@ lpf.2 cc: 30`, for a
  repeated name on the attached line). Ordinals are re-derived per
  document revision; the editor rewrites them on reorder like any
  write-back, and positional `TweakId` provenance remains the
  runtime fallback beneath them.
- CLASSIFICATION follows the Decided first-token rule: `midi` and a
  `label "." param` head are ADDRESSED (position-free, groupable
  anywhere — typically a controller map at the end of the file);
  everything else is POSITIONAL and must resolve against its attach
  target.

Validation is the editor layer's, lint-severity, never language
errors — the language ignores every directive regardless:
`unknown-directive-site` (positional first token is neither a
call-site name nor a parameter keyword of the attach target, nor
`name`, nor a label head), `unknown-parameter` (keyword not among
the site's declared parameters), `unknown-label` (dangling
addressed reference), `duplicate-label` (labels, below),
`ambiguous-selector` (selector resolution above — a bare selector
matching several candidates, resolved concretely by the ordinal
segment or diagnosed), `cc-out-of-range`, `reserved-key`.

**Labels as preferred provenance — the binding key is the FULL
resolved selector path, not (label, param).** A labeled binding's
document-scoped identity is

```rust
pub struct BindingKey { label: LabelId,
    site: Option<(SymId /*call-site name*/, u16 /*ordinal, 1-based*/)>,
    param: KwId }
```

`site: None` identifies a DECLARED-DEFINITION parameter
(`analog.cutoff` under an `inst`/`fn`/`bus` label); `site:
Some((name, n))` identifies the parameter OF THE SELECTED CALL
SITE, with the ordinal defaulting to 1 — so `hats.lpf` -> `(hats,
Some((lpf, 1)), cutoff)` and `hats.hpf` -> `(hats, Some((hpf, 1)),
cutoff)` are DISTINCT keys despite the shared parameter keyword,
and `hats.lpf.1` vs `hats.lpf.2` differ in the ordinal (Astra's
collision counterexamples). Persistence (directive text and the
external session file), overlays, and MIDI mappings are all keyed
by the full `BindingKey`; the earlier two-component `(label,
param)` identity is WITHDRAWN. Stability is per COMPONENT, stated
precisely rather than as a blanket "skips re-keying": the `label`
component is position-free and survives edits that move lines (the
improvement over positional `TweakId`s); the `param` component is
lexical; the ORDINAL component is re-derived per document revision
by mapping the previously resolved site's span through the
`doc-changed` change set (14.4) and re-counting same-named
occurrences — a pure reorder of same-named sites therefore
MIGRATES each binding with its site, and an edit that makes the
mapping ambiguous (the mapped span no longer lands on a same-named
call site) marks the binding STALE for explicit re-confirmation
instead of guessing. Unlabeled sites keep the existing positional
`TweakId` + generation scheme as the fallback, exactly as the spec
states; it also backs the stale-recovery path above. Named things
are implicit labels; the session resolves `analog.cutoff` through
the `LabelRegistry`.
LABEL RESOLUTION AND UNIQUENESS: the label namespace is per
document — explicit labels (the trailing `label:` short form,
`#@ name X`, and the block-following form for bodies) plus implicit
labels (every top-level `let`/`fn`/`inst`/`bus`/`look` name and
every named slot). Resolution is exact match over that single
namespace with no precedence between explicit and implicit labels:
a collision — explicit vs explicit, or explicit vs implicit — is
the editor diagnostic `duplicate-label`; an addressed reference to
the ambiguous name is rejected with the same diagnostic rather than
guessed, and the affected sites fall back to positional provenance
until one side is renamed. A label naming a LINE with several call
sites addresses them all for panel membership; selecting one call
site or parameter under the label follows the ADDRESSED SELECTOR
RESOLUTION rules above (`label.site`, `label.param`, ordinal
`label.site.n` for repeated names; `ambiguous-selector` when no
concrete selector is given).

**Write-back of learned mappings.** MIDI-learn on a directive-backed
binding WRITES the learned CC number back into the directive comment
— a validated text edit through the same `edit_epoch` +
mapped-span-verification path as literal write-back (14.4), so the
file carries the whole setup for a set, per the Decided requirement.
In EXTERNAL persistence mode the same learn event updates the
session file instead; nothing else differs.

**DAW-style parameter editors (Decided).** Every builtin declares
per-parameter EDITOR METADATA in the prelude signature table:

```rust
pub struct ParamMeta { ctl: CtlId, range: (f32, f32), curve: Curve,
                       unit: Unit, group: GroupId }
pub enum EditorKind { EqCurve, FilterResponse, DynamicsTransfer { multiband: bool },
    EnvelopeShape, DelayTaps, ReverbRoom, SamplerWave, WavetableFrames,
    GranularRegion, LfoShape, StereoField, XyPad,
    EuclidRing, ProbabilityDial, LengthHandle, Scalar }
pub struct EditorDecl { kind: EditorKind, params: Box<[ParamMeta]> }  // per builtin
```

The editor renders the meaning-matched editor for a call site chosen
from the SAME site enumeration as the sliders: EQ family gets the
draggable band curve with the LIVE SPECTRUM analyzer (12.5 cells)
drawn behind it; filters the response curve; dynamics the transfer
curve with live gain-reduction metering; envelopes the stage shape;
delay the beat-aligned taps; the SAMPLER waveform per the Decided
requirements (architecture.md Editor Requirements, sampler item):
draggable start/end/loop handles (the `begin`/`end` literal
sites); a slice GRID for `slice n` and `chop`/`striate` counts as
overlays; MANUAL slice markers drawn at the point literals and
DRAGGABLE — each marker IS one of the existing numeric sites
(pattern literals), so dragging goes through the standard
source-edit/overlay write-back with `BindingKey`/positional
provenance, nothing new; CLICKING A SLICE "sets the index pattern
step" RECONCILED with the code-only sequence rule: the click
writes a NEW NUMERIC VALUE into the currently selected, ALREADY
EXISTING index literal of the index pattern (the same validated
write-back a slider uses) — it never adds, removes, or reorders
steps, so sequence STRUCTURE stays code-only; with no index
literal selected the click is a no-op with a hint; the bank index
`n` opens the sample browser with a per-entry waveform preview;
wavetable/granular keep their frame/region views; signals the LFO shape; pan the stereo
field; any two parameters an XY pad; `euclid` a ring with draggable
hits/steps/rotation, `maybe` a probability dial, `hold`/`fast`/`slow`
a length handle. EVERY handle writes back to the same numeric sites
as a slider (source-edit or overlay, unchanged machinery) and every
handle is MIDI-learnable; the code never changes shape because of the
editor used. Packages' user-defined effects inherit `Scalar` editors
from their parameters unless composed of builtins whose editors
apply. **Sequences are code-only, per the Decided rule**: the step
grid and piano roll are DISPLAYS rendered from playing-state
telemetry (`PlayingEvent` `SrcRef`s and the checker's AST), never
editors of the list — the editable-numeric vs display-only-sequence
distinction is preserved structurally: display components have no
write-back path at all.

## 14. Session, REPL, LSP

### 14.1 Session

One `Session` object on the evaluator thread owns: interner, namespace,
checker state + host manifest, slot table, staging buffers, clock,
scheduler, pattern RNG seed, tweak table, form generations and the
reactive dependency graph (5.6), diagnostics bus, telemetry queue, and the
capability hosts. Everything user-facing goes through:

```rust
impl Session {
  pub fn eval(&mut self, src: &str, file: FileId, doc_revision: u64) -> EvalOutcome;
  pub fn tick(&mut self, host_now: f64);                            // scheduler step
  pub fn apply(&mut self, msg: ClientMsg) -> Vec<ServerMsg>;        // protocol
}
```

`EvalOutcome` carries the value (for the console), static diagnostics,
and the refreshed tweak-site table. Runtime failures arrive later on
the diagnostics bus with slot + beat, cleared when the next cycle of
that slot succeeds.

### 14.2 REPL

`vactr repl`: line editor over the same `Session`, console transcript
semantics: `_1`, `_2`, … are console-only registers (reader accepts
them only for `FileId::Console`), written only by expressions that
complete. `print` returns its argument. The REPL is the first
deliverable that plays sound: it drives the native host directly.

### 14.3 LSP

`vactr lsp` (feature `lsp`): tower-lsp server wrapping the SAME
reader + checker modules — diagnostics, hover (types from `TypedInfo`),
completion (prelude, namespace, keyword sets from the host manifest),
formatting. It connects to the live session's socket when one is
running so runtime diagnostics (slot, beat) appear in external editors
too; standalone it serves static diagnostics only. The dedicated editor
does NOT use LSP framing — it speaks the session protocol directly
(richer: telemetry, tweaks), while sharing the same underlying data.

### 14.4 Editor-runtime protocol

JSON messages (serde), transport: WebSocket (native session, feature
`host-native`) or direct wasm-bindgen calls (browser — same shapes, no
socket). Versioned envelope `{v, seq, kind, body}`.

| Direction | Message | Body |
|-----------|---------|------|
| C→S | `eval` | code (the full document text, 14.5.6), file, span (selects forms; flash + origin), `doc_revision`, `edit_epoch` (14.5.6) |
| C→S | `hush` / `stop` | (slot) |
| C→S | `set-var` | name, value, `defining_form_gen` (authority check), `edit_epoch` |
| C→S | `doc-changed` | file, `doc_revision`, base `doc_revision`, dirty spans (new-revision bytes) + change set, `edit_epoch` |
| C→S | `set-tweak` | tweak id, `form_gen`, value, `edit_epoch` |
| C→S | `subscribe` | telemetry, levels, diagnostics |
| C→S | `manifest?` | request sample/synth/control sets |
| S→C | `eval-result` | value text, static diagnostics, tweak sites (tier, `form_gen`, `doc_revision`), directive table + editor-validation diagnostics (13.5) |
| S→C | `stale-binding` | rejected tweak id, current `form_gen` |
| S→C | `diag` | runtime diagnostic (src ref, slot, beat, message) + clears |
| S→C | `playing` | batch of `PlayingEvent` (slot, beat, host time, `SrcRef`, dur) |
| S→C | `levels` | per-slot meter values (rate-limited) |
| S→C | `tempo` | bpm, beats per cycle, current cycle/beat |
| S→C | `bindings` | ONE batch per completed reactive pass (5.6): changed top-level names + display values, refreshed tweak sites for rebuilt forms, per-form failure diagnostics — never emitted mid-pass |

`playing` events carry host-clock timestamps; the editor schedules
highlights against the shared audio clock (`AudioContext.currentTime`
in the browser; the host `now()` natively), so highlighting is
sample-independent of message jitter. Spans in `playing` and `diag` are
tagged with the `doc_revision` they were computed against; the editor
maps them through its change history (CodeMirror change-set mapping) to
current positions and drops unmappable highlights, so editing above a
playing pattern never highlights unrelated text. Steps from lists
without element provenance (5.4) highlight the pattern's binding span
instead.

**Edits invalidate binding authority BEFORE any dependent write, by
an EDIT EPOCH carried on every write — not by debounce timing.**
`FormGen` changes only on evaluation, so it alone cannot detect an
edit that has not been evaluated yet. The editor maintains a
monotonic `edit_epoch` that increments on EVERY local edit
(immediately, before any debounce), and stamps every `set-tweak` and
`set-var` with the `edit_epoch` current when the control input was
produced. The debounce delays only the `doc-changed` NOTIFICATION,
never the epoch: the session tracks the highest `edit_epoch` it has
observed from any message, and REJECTS a `set-tweak`/`set-var` whose
stamped `edit_epoch` is older than a pending, not-yet-reconciled edit
to that site's form. Concretely, the ordered protocol is: (1) the
editor MUST send `doc-changed` for a form's dirty region before, or
in the same transport-ordered batch as, any controller write produced
after that edit — the editor flushes the pending `doc-changed` ahead
of a dependent write rather than waiting out the debounce; (2) the
session, on `doc-changed`, marks intersecting tweak sites and edited
`let`/`var` definitions EDIT-INVALIDATED (rejecting their writes with
`stale-binding` until re-eval brings fresh sites); (3) a `set-tweak`
arriving with an `edit_epoch` newer than the last `doc-changed` the
session holds for that form is itself treated as evidence of an
unreconciled edit and rejected until the matching `doc-changed`
arrives, closing the debounce race in the runtime rather than trusting
delivery order alone. Sites and definitions outside the dirty regions
stay valid: editing one form never kills unrelated bindings.
Independently, `set-var` still carries `defining_form_gen`; a name
redefined by a different form rejects the write as before.

**Dirty-span coordinates are revision-relative and mapped in the
runtime.** `doc-changed` carries its `doc_revision`, its BASE
`doc_revision` (the revision the edit applied to), and the dirty
spans AS BYTE RANGES IN THE NEW REVISION plus the change set (offset
deltas) from base to new. The session keeps each tweak site's span in
the revision it was last evaluated under and, on `doc-changed`, maps
those stored spans forward through the change set to the new revision
before testing intersection — so an insertion above an unchanged site
shifts the site's runtime coordinates to match, and a later edit to
that shifted site intersects correctly. Successive coordinate-changing
edits compose their change sets in order. This makes runtime authority
coordinates track edits, rather than relying on the editor's
highlight mapping (which is display-only). Planned tests (TASK-009,
TASK-010): a debounce-race write stamped with a newer `edit_epoch`
than the session's last `doc-changed` is rejected; an insertion
before a site followed by an edit to the shifted site invalidates the
correct site through composed change-set mapping.

### 14.5 Session-layer implementation decisions (TASK-009, 2026-09-25)

This section fixes the choices that sections 5.7, 12.3, 13.5, 14.1-14.4
and 17 leave open for issue #4 (Session, protocol, packages, directives,
REPL, LSP, CLI and the self-analysis surfaces). It follows the shape of
12.8 and changes no Decided behavior. The wire format of every message
and the CLI verbs are in `design-docs/specs/command.md`; this section
holds the rules. Open author questions are in
`design-docs/user-qa/pending-session-questions.md` (S1-S6). The
implementation follows each recommendation until it is answered.

#### 14.5.1 Scope boundary

In scope: every TASK-009 deliverable and completion criterion of
`impl-plans/active/vactr-core.md` plus the 12.3 amendment surfaces.
These are the boundary decisions:

- **Session wraps, never re-implements.** `Session` owns one `Evaluator`
  and one `sched::Runtime` (12.8.1) and adds document state, package
  state, directive tables, console registers, write authority and the
  outbox. It adds no second evaluator loop and no second scheduler.
- **Fetching happens only in `vactr get`.** A running session reads
  packages from `vactr.lock` and the verified cache and never touches
  the network, so the evaluator can never block on it (5.7 failure
  contract). If an import is not in the lock, the result is a load
  diagnostic that says to run `vactr get`. The editor's import action
  (TASK-010) calls the same resolve-and-fetch code on its IO side.
- **Browser package store: Rust half only.** TASK-009 implements the
  proxy protocol, validation, digest and staged publication against
  two traits: `ProxyTransport` (GET a URL) and `CacheBackend` (staging
  directory, atomic rename, stamp). A local HTTP fixture exercises them.
  The `fetch()` transport and the OPFS backend belong to TASK-010's TS
  shell, because the raw wasm ABI (12.8.10) has no JS imports.
- **Native package stores:** a git-tag store that shells out to `git`
  (14.5.7) and a local-directory store. TASK-009 has no native HTTP
  client.
- **Self-analysis: live taps are native-tier, for `:master` and named
  buses only.** The browser taps are TASK-010. The slot sources
  `:d1..:d9` and the input `:in` report "not available on this host"
  (S2). The source forms `fft :src` and `amp :src` (design-music lines
  89-90) are not in issue #4's list. `fft n` and bare `amp` stay as they
  are (S2).
- **No `Engine::render` API.** 12.3 names `Engine::render`, but TASK-008
  shipped `NativeAudioHost::headless` + `AudioSide::render` as the
  offline path, and that is what offline `render` uses (14.5.9).
  `dsp/engine.rs` (786 lines) is not touched.

#### 14.5.2 Dependencies and features

| Crate | Gate | Why |
|-------|------|-----|
| `serde` (derive), `serde_json` | regular | protocol codec (14.4 says serde). Both are wasm32-clean and are also required by tower-lsp |
| `miniz_oxide` | regular | raw-deflate inflate for proxy zips. Pure Rust, wasm32-clean. The zip container reader is in-crate |
| `tungstenite` (default features, no TLS) | `host-native`, non-wasm32 target table | session socket server and the LSP attach client |
| `getrandom` | `host-native`, non-wasm32 | session token. Already in tungstenite's tree |
| `tower-lsp`, `tokio` (rt, io-std, macros, sync) | `lsp`, non-wasm32 | `vactr lsp` |

- `lsp = ["host-native", "dep:tower-lsp", "dep:tokio"]`, because the LSP
  attach client uses tungstenite.
- SHA-256 is in-crate (`pkg/sha256.rs`) and is tested against the FIPS
  180-4 vectors: empty, `abc`, the 448-bit message and one million `a`.
- `vactr.toml` uses an in-crate TOML subset parser (14.5.7).
- No git library, no HTTP client, no zip crate, no CLI-parser crate and
  no line-editor crate.
- Versions follow the 12.8.10 policy: the highest releases whose whole
  resolved tree builds on Rust 1.83, with `cargo update --precise` where
  needed. The chosen versions, the `Cargo.lock` diff and a `cargo audit`
  run go in the CONTRACTS evidence.
- `cargo tree -e normal --target wasm32-unknown-unknown` must list none
  of tungstenite, getrandom, tokio or tower-lsp.
- **Gating of modules.**
  - `cli/ws.rs` (the session socket server): `all(feature = "host-native", not(target_arch = "wasm32"))`,
    declared from the CLI-owned `cli/mod.rs`.
  - `pkg/native/` and `cli/`: `not(target_arch = "wasm32")`. They use
    `std::fs` and `std::process`.
  - `lsp/`: `all(feature = "lsp", not(target_arch = "wasm32"))`.
  - `src/main.rs` keeps an empty wasm32 `main` so the default-feature
    wasm32 build still links the bin target.
  - Every other new module is core and stays wasm-safe (6.5.7).

#### 14.5.3 Files and the 800-line budget

No `.rs` file may reach 800 lines (hard limit 1000). A wave that pushes
a file it touches to 800 or more lines splits that file in the same
wave. Tests go in `tests/` submodules, as in 6.5, 7.1 and 12.8.

**Ownership rule.**
- A file has exactly ONE writer per wave.
- A module is declared by the wave that owns its parent `mod.rs`, in
  the same wave that creates the file, or earlier with a stub.
- A file may pass between SEQUENTIAL waves only in the form "CONTRACTS
  creates a stub or seed, a later wave fills it". Both plans then list
  that file in their `writePaths`. Two plans of the same wave never
  share a file.
- Each NEW top-level module belongs wholly to one wave, `mod.rs`
  included. CONTRACTS adds only the `src/lib.rs` declarations and an
  empty `mod.rs` stub for each, so the crate compiles between waves.
  The one exception is `session/mod.rs`. CONTRACTS seeds it with
  `pub mod changes;`, because DIRECTIVES (wave 2) uses `ChangeSet`.
  SESSION then adds its own declarations, and both plans list the file.
- Every seeded or stubbed file with a later writer is listed here:
  - `session/mod.rs` (CONTRACTS -> SESSION)
  - `ns/eval_doc.rs` (CONTRACTS -> SESSION)
  - `value/sample.rs` (CONTRACTS -> ANALYSIS, for the playback and
    analysis helpers)
  - `host/native/tap.rs` (CONTRACTS -> ANALYSIS)

  `ns/stage.rs` and `host/caps.rs` are written only by CONTRACTS, which
  lands their final variant shapes and default method bodies.
  - the `pkg/`, `directives/`, `cli/` and `lsp/` `mod.rs` stubs
    (CONTRACTS -> their owner)

| Module | Files (owner wave) |
|--------|--------------------|
| `session/` | `changes.rs` (`ChangeSet`, which DIRECTIVES and SESSION both use) and the seed `mod.rs` declaring it (CONTRACTS); `mod.rs` (extended), `session.rs` (`Session`, `SessionConfig`, per-file `DocState`), `eval.rs` (document pipeline, `alias_env_for`), `publish.rs` (`bindings` batches), `authority.rs` (edit epochs, invalidation, write validation), `protocol.rs` (`ClientMsg`/`ServerMsg`/envelope), `codec.rs` (JSON), `console.rs` (registers, transcript), `repl.rs` (line loop over `BufRead`/`Write`), `tests/` (SESSION). SESSION owns every file here except `changes.rs`, and `session/` has no socket file |
| `pkg/` | `mod.rs`, `semver.rs`, `manifest.rs`, `lock.rs`, `mvs.rs`, `sha256.rs`, `digest.rs`, `validate.rs`, `zip.rs`, `store.rs` (`PackageStore`, `PkgSources`, `PkgError`), `proxy.rs`, `cache.rs`, `load.rs`, `native/{mod,dir_store,git_store,fs_cache}.rs`, `tests/` (PKG) |
| `directives/` | `mod.rs`, `parse.rs`, `attach.rs`, `labels.rs`, `resolve.rs`, `key.rs`, `persist.rs`, `writeback.rs`, `tests/` (DIRECTIVES) |
| `cli/` | `mod.rs`, `args.rs`, `repl.rs`, `run.rs`, `serve.rs`, `get.rs`, `ws.rs` (the session socket server) (CLI) |
| `lsp/` | `mod.rs`, `server.rs`, `analysis.rs`, `convert.rs` (LSP) |
| language | `compile/compiler.rs` split into `compiler.rs` + `names.rs` (it is at 798 lines; `compile/mod.rs` declares the new file) plus console-register resolution; `ns/namespace.rs` (console register slots); `value/mod.rs`, `ns/mod.rs` and `host/native/mod.rs` (declarations of the new stubs); `types/diag.rs`, `vm/fail.rs` (the codes of 14.5.12); `value/value.rs`, `value/sample.rs`, `value/print.rs`, `value/eq.rs` (`Sound::Buffer`); `ns/stage.rs` (`StagedEffect::{Capture, Render}`); `host/caps.rs` (tap default methods, `SampleLoader::register_bank` default); `host/native/tap.rs` stub + declaration (CONTRACTS). `ns/pkg.rs` (package evaluation into `PkgNs`), `host/native/loader.rs` (asset banks) (PKG). `ns/eval_doc.rs` (`impl Evaluator`: eval with a manifest and a doc revision; `evaluator.rs` is at 777 lines and is not edited) (SESSION). Self-analysis files (ANALYSIS): `value/sample.rs` (fills the CONTRACTS seed), `types/natives_domain.rs`, `types/natives.rs`, `types/infer_call.rs` (split if it reaches 800), `vm/natives/{mod,analysis,tex}.rs`, `dsp/{mod,offline,caps,bus,fft}.rs`, `dsp/effects/analyzer.rs` (visibility only), `sched/{mod,offline,tap,runtime,commit}.rs`, `ns/insts.rs`, `host/native/{audio,tap}.rs`, `host/testing.rs` |
| other | `Cargo.toml`, `Cargo.lock`, `src/lib.rs` (CONTRACTS); `src/main.rs`, `tests/cli.rs` (CLI); `tests/lsp_smoke.rs` (LSP); `tests/directive_fixtures.rs`, `tests/fixtures/directives/vocabulary.toml` (DIRECTIVES); `tests/fixtures/spec/manifest.toml` (ANALYSIS for design-music ordinal 2, FINAL for lang-reference ordinal 5; the waves never run in parallel), `tests/support/eval.rs` (FINAL) |

#### 14.5.4 Session and the eval pipeline

- **Construction.** `Session::new(cfg: SessionConfig, hosts: Hosts) ->
  Session`. The config holds:
  - the `CapabilitySet` tier;
  - the project root, where `vactr.toml` and `vactr.lock` live;
  - the package cache;
  - the `PersistenceMode` (`Directive` by default, 13.5).

  `Session::new` builds the Evaluator + Runtime pair exactly as 12.8.3
  describes, and registers the analysis context of 14.5.9 in the
  prelude.
- **Files.** Protocol `file` strings map to `FileId`s. The first file
  gets 1, the next 2, and so on; `FileId::CONSOLE` (0) is the REPL. Each
  file has a `DocState`: its last evaluated revision and text, its
  directive table, the authority state of 14.5.6, and the labels.
- **`eval(src, file, doc_revision) -> EvalOutcome`** does this, in order:
  1. Phase 1: `prescan_imports(src)`.
  2. Packages: every import is resolved through the lock and the
     verified cache and compiled into its `PkgNs` (14.5.7). This comes
     before any form runs, so no form waits.
  3. Phase 2: read the whole document with an `AliasEnv` holding the
     session prefixes plus every prescanned import. The session then
     walks the nodes and turns any `Atom::Qualified` whose prefix is
     bound only by an import that starts AFTER the atom into
     `unbound-qualifier`. This matches reading form by form with the
     env threaded through (5.7), without a reader change.
  4. Expand.
  5. Build the directive table (14.5.8) from the reader trivia.
  6. For each top-level form inside the requested span (the whole
     document when there is no span): check it against the SESSION
     `HostManifest` (the spec default plus package banks), evaluate it
     with `Evaluator::eval_form_in(form, &manifest, doc_revision)`,
     then `Runtime::drain`. Any queued `upd` then runs one
     `run_pass` (14.5.5).

  Draining after every form is what lets a later form of the same
  document see an offline `render` or a finished `capture` from an
  earlier form (14.5.9). Every `SrcRef` made by a form evaluated at
  revision r carries r.
- **`EvalOutcome`** holds:
  - per form: the span, the value text (the canonical print, 6.5.3),
    the failure as a diagnostic with origin, and the `form_gen`;
  - all static diagnostics: reader, expander, checker, load, package
    and directive lint;
  - the refreshed tweak-site table, tagged with `doc_revision`;
  - the directive table summary.

  A reader or expander error in one form does not stop the other forms.
  This differs from `eval_str`, which stops at the first error.
- **`tick(host_now) -> Vec<ServerMsg>`** does this, in order: apply the
  coalesced writes (14.5.6), run `Runtime::tick`, poll captures, then
  emit `diag`, `playing` (subscribers only), `levels` (at most 10 per
  second) and `tempo` (on change).
- **`apply(msg) -> Vec<ServerMsg>`** returns replies and broadcasts in
  order. The transport routes by kind:
  - to the requester only: `eval-result`, `stale-binding`,
    `directive-edit`, `manifest`, `protocol-error`;
  - to every subscriber: `bindings`, `diag`, `playing`, `levels`,
    `tempo`.
- **A live session never dies (17, invariant 5).** Every failure class becomes a
  diagnostic or a failure with its origin. A panic is a bug, and a
  no-panic test covers malformed protocol input.

#### 14.5.5 Reactive publication

- **One batch per completed pass, built after the pass.** A pass is
  complete when `run_pass`, or a `set_tweak` that returns
  `Some(PassReport)`, returns. By then 5.6 validation and rollback have
  already run. `publish.rs` builds exactly ONE `bindings` message from
  the `PassReport`:
  - `changed`: the names whose committed value changed in the pass,
    each with its display value and defining `form_gen`
    (`PassReport.bindings`);
  - `sites`: the refreshed tweak-site table of every rebuilt form;
  - `states`: EVERY form scheduled in the pass, with its final state,
    one of:
    - `ok`: clears a badge;
    - `failed`: with the diagnostic and the committed, restored display
      value;
    - `blocked`: with `blocked_on` and the committed display value.
- **Pass boundaries.** A pass that scheduled no form publishes nothing.
  Because the batch is built from the returned report, nothing can be
  published mid-pass or mid-round. `PassEvent`s are never forwarded, so
  a provisional value, a staged bind, a revocation or a cell update of
  a rolled-back form cannot reach a subscriber. The recording host
  proves the host half.
- **Recovery batches.** Equal-value repairs and unblocking both publish
  the affected forms as `ok`, even when no value changed. Badges are
  state, not value (5.6 joins rule).
- **Ordering.** `eval-result` comes before the `bindings` batch of any
  pass that the eval triggered.

#### 14.5.6 Protocol codec and write authority

- **Envelope.** `{v, seq, kind, body}` with an optional `re` (the seq of
  the client message a reply answers). The version is `v = 1`.
  - Unknown `v`, unknown `kind`, a malformed body or invalid JSON each
    produce `protocol-error` (`unsupported-version`, `unknown-kind`,
    `bad-body`, `bad-json`), and the connection stays open.
  - Frames larger than 1 MiB are refused.
- **Additions to the 14.4 table.** These are needed by the criteria and
  add nothing else:
  - `protocol-error` (S→C).
  - `learn` (C→S: binding, cc, ch, `edit_epoch`) and `directive-edit`
    (S→C: file, `doc_revision`, span, expected text, new text) carry
    the 13.5 MIDI-learn write-back. The editor applies a
    `directive-edit` only after it checks the expected text, and then
    sends `doc-changed` as usual.
  - `manifest` (S→C), the reply to `manifest?`.
- **Meaning of `eval` fields.** `eval.code` is the FULL document text
  at `doc_revision`, the optional `span` selects the forms to run, and
  `edit_epoch` is the editor epoch that the text reflects (rule 3).
  The session needs the whole text for the directive table and the
  phase-1 `AliasEnv` (5.7, a single-form eval uses the buffer's scan).
- **Value encoding.** `set-tweak` and `set-var` values are JSON numbers,
  coerced to the site's `NumTy` or to the var's current type, or
  booleans. Anything else is `bad-body`. Display values are canonical
  prints. Beats are `[num, den]` pairs, and host times are f64 seconds.
- **Change sets.** A `ChangeSet` is a sorted list of non-overlapping
  replacements `{from, to, insert_len}` in base-revision byte offsets.
  `map_span` shifts a span that lies entirely before or after every
  replacement and reports `Touched` otherwise. Composition applies the
  maps in order.
- **Authority rules**, applied when a write arrives AND again when the
  coalesced write is applied at the tick:
  1. `set-tweak`: the site's `form_gen` must equal the message's
     `form_gen`, else `stale-binding` with reason `stale-form-gen`.
  2. The site must not be edit-invalidated, else reason
     `edit-invalidated`.
  3. Unreconciled edit: `edit_epoch` is per editor document and
     monotonic. The session keeps a RECONCILED epoch per document: the
     highest `edit_epoch` carried by any `eval` or `doc-changed` for
     that document. `eval` carries the epoch its text reflects, which
     is an addition to the 14.4 table. Without it, edits made before
     the first eval would look unreconciled forever. A write whose
     `edit_epoch` is higher than the reconciled epoch is rejected with
     reason `unreconciled-edit`, and stays rejected until the matching
     `doc-changed` or `eval` arrives (14.4 rule 3).
  4. `set-var`: rules 2 and 3 apply to the name's defining span. The
     name's CURRENT defining form must have `defining_form_gen`, else
     reason `superseded-definition`.
  5. `doc-changed` whose base revision equals the session's revision
     for the file: every stored site and definition span is mapped
     forward, and any span that is `Touched` or intersects a dirty span
     is edit-invalidated. Every other span moves to its mapped
     coordinates, and the stored revision advances.
  6. `doc-changed` whose base revision is not the session's revision:
     the session cannot map it, so every site and definition in the
     file is edit-invalidated until the next eval. It never guesses.
  7. An eval of the file clears invalidation for the forms it rebuilds,
     through fresh sites.
- **Coalescing (13).** An accepted write is stored latest-wins per
  target (a `TweakId` or a name) until the next tick. The tick applies
  all pending `set-var`s as `queue_upd` plus ONE `run_pass`, then each
  pending `set-tweak` through `Evaluator::set_tweak`, whose `reeval`
  rebuild is its own pass and its own batch.
- **Tiers.** `direct`, `reeval` and `manual` keep the meanings they
  already have in the evaluator (13, 5.6). A `manual` site updates its
  slot with no replay and reports `manual` in the site table.

#### 14.5.7 Packages

- **`vactr.toml`** is an in-crate strict TOML subset: tables
  `[package]` and `[deps]`, basic strings, string arrays and `#`
  comments. Anything else is a manifest parse error.
  - `[package]` keys: `path` (the `PackageId`, which must equal the
    import path it was fetched as), `vactr` (the minimum language
    semver) and `assets` (relative directories).
  - `[deps]`: `"github.com/o/n" = "v1.2.0"`.
  - A set's ROOT manifest may omit `[package]`. `vactr get` creates
    `./vactr.toml` with only `[deps]` when none exists.
- **`vactr.lock`** is line-based and deterministic. The first line is
  `# vactr.lock v1`, followed by one line per package,
  `<path> <version> sha256:<64 lowercase hex>`, sorted bytewise by path.
  An unknown header version or a malformed line is a load diagnostic.
- **Semver and MVS.** Versions are tags `vMAJOR.MINOR.PATCH[-pre]` with
  semver 2.0 precedence; other tags are ignored. MVS follows 5.7: a
  breadth-first walk of the requirement graph over each visited
  version's manifest, then the maximum of the minimums per path. With
  no version given, `vactr get` picks the highest non-prerelease tag.
  Semantic-import-versioning (`/v2` paths) is not in v1: majors compare
  like any other version (S5).
- **Stores (`PackageStore`, IO side, never on the evaluator).** The
  trait has `list_versions(id)`, `manifest(id, version)` and
  `fetch_into(id, version, staging)`.
  - LOCAL-DIRECTORY store: `<root>/<path>@<version>/`, used by fixtures
    and by `vactr get --store dir:<root>`.
  - GIT store: runs the `git` binary with fixed argv arrays (no shell),
    `--` before operands, the environment `GIT_TERMINAL_PROMPT=0` and
    `GIT_CONFIG_NOSYSTEM=1`, and `-c core.hooksPath=/dev/null
    -c protocol.file.allow=user`. Tags come from `ls-remote --tags
    <base>/<path>`, and the fetch is `clone --depth 1 --branch <tag>
    --single-branch`. `.git/` is removed before validation and never
    takes part in the digest.
    - `<base>` is a constructor argument, `GitStore::new(base)`. The CLI
      always passes `https://` and has no flag to change it. The PKG
      test passes a `file://` URL for a temporary directory holding
      local bare repositories, `<tmp>/github.com/o/n`, that the test
      creates with the `git` CLI, so the test never reaches the public
      network.
  - PROXY store (`proxy.rs`): `{proxy}/{path}/@v/list`,
    `{proxy}/{path}/@v/{version}.toml` (the manifest, so an MVS walk
    never downloads zips) and `{proxy}/{path}/@v/{version}.zip`, over
    `ProxyTransport`.
- **Validation (5.7 revised), run first on every store** over the
  entries of a staged tree or the entries of a zip:
  - Each path is relative, uses `/`, is UTF-8, contains no byte below
    0x20 and no 0x7f, and has no empty, `.` or `..` segment and no
    drive or absolute prefix.
  - Only regular files are allowed. Symlinks are refused (tree:
    `symlink_metadata`; zip: Unix mode bits in the external attributes).
    So are devices and hardlinks.
  - Exact duplicates and duplicates under Unicode simple case folding
    are refused.
  - Default caps: 10 000 entries and 256 MiB uncompressed, checked
    during extraction.
  - Zip: only stored and deflate entries are allowed; encryption, zip64
    and multi-disk archives are refused. The declared size is enforced
    against the inflated byte count.
  - Manifest `assets` must normalize strictly under the root.

  A violation is `package-integrity` naming the entry, and the cache is
  not changed.
- **Digest.** `digest.rs` implements the 5.7 record exactly,
  `u32_be(len) || path || sha256(contents)` over bytewise-sorted paths,
  with the lock digest being the sha256 of the concatenation. A tree
  and a zip of the same sources give the same digest (portability
  test).
- **Staged, atomic publication (`cache.rs`).** The sequence is: fetch or
  extract into `<cache>/.staging/<random>` (created with
  `create_dir`, never reused), validate, digest, compare with the lock
  (or record the digest when `vactr get` writes the lock), write the
  stamp `<entry>/.vactr-digest`, and finally `rename` into
  `<cache>/<path>@<version>`. Any earlier failure removes the staging
  directory. On a verified read, the stamp must equal the lock digest.
  A mismatch is `package-integrity` and the session never re-hashes
  while it plays. The cache root is `$VACTR_HOME/pkg`, with
  `VACTR_HOME` defaulting to `~/.vactr`. An interrupted-extraction
  test injects a failure after half the entries and asserts that no
  entry path exists.
- **Loading (`ns/pkg.rs`, `pkg/load.rs`).** A package's `.vact` files,
  in bytewise path order, go through read -> expand -> check -> compile
  -> run into a fresh `PkgNs`. Each file gets its own `FileId`, so
  diagnostics are attributed to the package. The package's forms are
  ordinary forms: they share the session prelude, instrument registry
  and effect sink. Its own imports resolve through the same lock.
  Re-import replaces the `PkgNs` (5.7).
- **Assets.** Each immediate subdirectory `<bank>` of a manifest asset
  directory registers, through `SampleLoader::register_bank`, as the
  bank `:<name>-<bank>`, where `<name>` is the package's default prefix.
  That keyword joins the session `HostManifest` (S3). A clash with an
  existing bank is `import-collision` (w), and the first registration
  wins. Path literals inside package files resolve relative to the
  file, contained under the package root.
- **Failure contract.** The load diagnostics are:
  - `package-not-locked`: the import is not in the lock;
  - `package-not-fetched`: the lock entry has no verified cache entry;
  - `package-integrity`;
  - `package-load-failed`: a manifest parse error or the package's own
    compile errors, which are also listed at their package origins.

  Each sits at the import span. The prefix stays bound-but-broken, so
  qualified uses under it become `undefined-name`, and the session keeps
  playing.
- **LSP analysis (no execution).** The LSP uses the same `alias_env_for`.
  A fetched package's qualified names are typed by a check-only pass
  over its cached sources. An unfetched package types them as `any`,
  with the warning `package-not-fetched`.

#### 14.5.8 Directives and binding persistence

- **Parse (`parse.rs`)** implements the 13.5 PROPOSED grammar as
  written. It classifies each directive by its first token: `midi` and
  `label.sel[.n]` are Addressed; everything else is Positional.
  `#@ name X` and a leading `ident:` define labels. `cc:` takes ints
  and `_`, and `ch:` takes an int. `range:` gives `reserved-key`, and a
  CC outside 0..127 or a channel outside 1..16 gives `cc-out-of-range`.
- **Attach (`attach.rs`)**, per the Decided rule:
  - A trailing `#@` on a code line attaches to that line.
  - Consecutive own-line `#@` lines form one block, which attaches to
    the nearest PRECEDING statement, block or definition whose
    indentation is no deeper than the block's. At column 0 after an
    `inst` body, that means the whole `inst`.
  - A positional block with no preceding target gives
    `unknown-directive-site`.
  - Addressed directives are position-free.
  - `#@ midi ch: n` sets the file default. A later one replaces it,
    with `duplicate-key` (w).
- **Labels (`labels.rs`).** Explicit labels plus implicit ones (every
  top-level `let`/`fn`/`inst`/`bus`/`look` name and every named slot)
  share one per-document namespace. Any collision is `duplicate-label`.
  An addressed reference to the colliding name is rejected, and the
  affected sites fall back to positional provenance.
- **Resolve (`resolve.rs`)** follows 13.5 ADDRESSED SELECTOR RESOLUTION
  exactly: parameter first for definition targets, then call-site name,
  then ordinal. It reports `ambiguous-selector`, `unknown-label`,
  `unknown-parameter` and `unknown-directive-site`. Declared parameter
  order comes from the `inst`/`fn`/`bus` header nodes and, for
  builtins, from the `EditorDecl`/`ParamMeta` order. All of these are
  lint-severity warnings, and evaluation never reads the table.
- **Binding identity (`key.rs`).**
  - The key is `BindingKey { label, site: Option<(SymId, u16)>, param }`
    as in 13.5; an unlabeled site keeps `TweakId` + `form_gen`.
  - On `doc-changed`, each keyed site's last resolved span goes through
    the `ChangeSet` and the same-named call sites are counted again:
    - a clean mapping onto a same-named site MIGRATES the key (ordinal
      rewritten);
    - a `Touched` mapping, or one that does not land on a same-named
      site, marks the binding `stale`. It keeps its data and waits for
      re-confirmation; positional fallback is the recovery.
- **Persistence (`persist.rs`).** `trait BindingPersistence { fn
  load(&self, doc) -> BindingSet; fn save(&mut self, doc, &BindingSet)
  -> Persisted; }`. A `BindingSet` holds entries `{ key, panel: bool,
  midi: Option<{cc, ch}>, overlay: Option<f64> }`.
  - `Directive` (the default) renders panel membership and mappings as
    directive text edits, and never writes overlay values into the
    source (13).
  - `ExternalFile` writes `<doc>.bindings.json` (`{v: 1, bindings}`,
    with keys spelled `label.site.n.param`) and does keep overlays.

  The round-trip criterion covers keys, panel membership and mappings
  in both modes, plus overlays in ExternalFile mode (S4).
- **Write-back (`writeback.rs`).** A `learn` produces the minimal edit
  that inserts or replaces the `cc:` number at the binding's position
  in its directive, or appends a new directive block if none exists.
  The edit is validated like literal write-back:
  - the binding must not be stale;
  - the `edit_epoch` must pass rule 3 of 14.5.6;
  - the expected text is the directive's current text.

  In ExternalFile mode, `learn` updates the session file and returns no
  `directive-edit`.
- **Vocabulary fixtures** are in `tests/fixtures/directives/vocabulary.toml`,
  read by `tests/directive_fixtures.rs` through `tests/support`'s TOML
  subset parser. Each case carries `class = "authority-question"` and
  `question`: this is the vocabulary's authority-question channel, kept
  apart from the spec manifest so that the DIRECTIVES and ANALYSIS waves
  never write the same file.

#### 14.5.9 Self-analysis surfaces (12.3 amendment)

- **Sample values.** `Sound::Buffer(Rc<SampleBuf>)` is typed `sound`.
  - `SampleBuf` holds the sample rate, interleaved stereo `f32` frames,
    and a state: `Pending`, `Ready` or `Failed(Failure)`.
  - Reading a `Pending` buffer (analysis or playback) is
    `Failure(capture-pending)`. A `Failed` buffer returns its failure.
  - `s buf` plays a Ready buffer through the `sampler` route. At commit
    it is installed as a sample resource keyed by the buffer's
    identity, so `chop` and `granular` compose.
- **Surfaces and checker types.** `spectrum`, `scope` and `render` join
  the M1 fixed subject-overload group (7.1.4, B2):

  | Call | Type | Meaning |
  |------|------|---------|
  | `scope :src n` | `fn keyword int -> [float]` | the last n mono frames of a live tap, n <= 8192 |
  | `scope buf n` | `fn sound int -> [float]` | the first n mono frames of a buffer |
  | `spectrum :src bins: b` | `fn keyword -> [float]` (kw `bins`, default 64) | one FFT frame of 2b samples from the tap. b is a power of two in 8..2048, else `Failure(type)` |
  | `spectrum buf bins: b` | `fn sound -> [float]` | the same, averaged over consecutive frames of the buffer |
  | `spectrum` in a `bus`/`inst` body | unchanged | the analyzer unit (12.8.6 rule) |
  | `capture :src cycles` | `fn keyword any -> sound`, effect, needs `Analysis` | a `Pending` buffer that records the NEXT `cycles` cycles, starting at the next cycle boundary (S1) |
  | `render cycles` | `fn any -> sound`, effect | offline render; a number subject selects it |
  | `render` / `render :o0` | unchanged | the visual setting |
  | `rms buf` / `peak buf` | `fn sound -> float` | RMS / absolute peak over both channels |

  `dsp/offline.rs` runs the SAME `fft.rs` and analyzer helpers over a
  slice, so nothing forks.
- **Live taps (native).** `AudioSide::render` copies the `:master`
  output and each named bus's block into preallocated per-source
  history rings of 8192 mono frames. The copy is lock-free and
  allocation-free (the 12.8.9 probe covers it). The evaluator reads a
  seqlock snapshot through the `AudioHost` default methods
  `tap_snapshot` and `arm_capture`/`poll_capture`. `NoopHost` and the
  browser return `host-unavailable`; the recording host
  (`host/testing.rs`) returns a synthetic signal. A capture streams the
  armed source into an SPSC ring that `Runtime::tick` drains into the
  buffer. It is bounded by `max_capture_seconds`: a longer request is
  `beyond-capability` at the call. `dsp/bus.rs` gains the read-only
  bus-frame accessor.
- **Offline render (`sched/offline.rs`).** A `Render` effect staged by
  `render` is handled in `Runtime::drain`, where `&mut Evaluator` is
  free. The runtime:
  - builds a headless `NativeAudioHost` and a second `Runtime` with the
    same resolver and caps;
  - installs the live instrument and bus graphs and the ACTIVE slot
    bindings;
  - ticks a virtual clock from cycle 0 at the live tempo through the
    six 11.3 steps WITHOUT step 6 (no `at`/`once` thunk runs, so
    nothing leaks into the live session);
  - renders `cycles` cycles into the buffer and marks it `Ready`.

  The live runtime and clock are untouched. The work is bounded by
  `max_capture_seconds` and done synchronously on the evaluator thread,
  outside any audio callback. `CapabilitySet::native()` sets
  `offline_render = true`. The browser preset keeps it false, so
  `render n` fails AT THE CALL with `Failure(beyond-capability)` "not
  available on this host". Builds without `host-native` behave like the
  browser.
- **Analysis context.** `Session::new` registers an
  `AnalysisCx { caps, taps }` in the prelude, the same way
  `register_load` does, so natives can reach the caps and the tap
  snapshots without a new VM field. When no context is registered, as
  in a bare `Evaluator` or the spec fixture runner, every self-analysis
  native fails with `host-unavailable`, just as under `NoopHost`. That
  lets ANALYSIS repin design-music ordinal 2 without touching the
  runner.

#### 14.5.10 REPL, CLI and the session socket

- **REPL (`session/repl.rs` + `cli/repl.rs`).** A stdin reader thread
  sends lines over an mpsc channel. The main (evaluator) thread waits
  in `recv_timeout(TICK_PERIOD)` and ticks the session between lines,
  so sound keeps playing while the prompt waits.
  - Continuation: a line ending in the block colon, or an indented line
    or a `>` line that follows one, continues the entry. A blank line
    submits.
  - EOF exits with code 0.
  - A completed non-failing expression binds the next register `_n`
    (`Namespace` console slots; the compiler resolves
    `Atom::ConsoleReg`, and the checker keeps typing it `any`) and
    prints `_n = <value>`. A failure prints the diagnostic and binds
    nothing.
  - The transcript prints diagnostics, console output and runtime
    `diag`s as they arrive.
- **CLI.** Hand-written argument parsing. The verbs and flags are in
  `command.md`. The host is `--host native|noop`; the default is
  `native` when the crate is built with `host-native`. If the device
  fails to open, the CLI warns and falls back to `noop` (a live session
  never dies, S6). `--cycles N` with `noop` drives a VIRTUAL clock, so
  tests end deterministically.
- **Session socket (`cli/ws.rs`, 17).**
  - It binds `127.0.0.1` only; a non-loopback `--bind` is refused.
  - The token is 32 bytes from `getrandom`, hex-encoded, and printed
    once to stderr as the connect URL
    `ws://127.0.0.1:<port>/session?token=<hex>`.
  - The handshake requires the path `/session` and a constant-time
    token match. Anything else gets HTTP 401 before the upgrade.
  - At most 8 connections, and frames up to 1 MiB.
  - Each connection has one thread that reads with a 5 ms timeout,
    parses messages, and forwards them over mpsc to the evaluator
    thread; the same thread flushes that connection's outbound queue.
  - `Session` never leaves the evaluator thread (17.1).
  - The token is never written to disk.

#### 14.5.11 LSP

- **Threading.** tower-lsp over stdio on a current-thread tokio
  runtime. All `!Send` analysis state (interner, `Rc` values) lives on
  ONE analysis thread, and handlers talk to it through channels. This
  is required because tower-lsp futures are `Send`.
- **Features.**
  - Full-text sync.
  - `publishDiagnostics`: reader, expander, checker, package
    (`package-not-fetched` and so on) and directive lint.
  - Hover: the type of the innermost node at the cursor, from
    `CheckResult.types`, which is the `TypedInfo` of 14.3.
  - Completion: prelude names from `NativeTable`, document top-level
    names, manifest keywords, package prefixes and qualified names.
    **Revised 2026-09-30:** completion delegates to the shared
    `src/complete` engine (`design-completion.md` section 4). The
    engine keeps these sources and adds cursor contexts, scope-aware
    locals, ranking, a cap of 100 with `isIncomplete`, and UTF-16
    `textEdit` ranges.
  - Formatting: only strips trailing whitespace and ensures one final
    newline. This keeps every comment and directive byte-identical.
    **Revised 2026-09-30:** `format_edits` now delegates to the
    `src/fmt` formatter (`design-formatter-and-syntax.md` 3.7.2). Those
    whitespace rules are kept (R5, R7, R8). Continuation and
    comment-only lines are also re-indented. The comment text is kept,
    and `#@` bindings are preserved. A document with reader errors gets
    no edits.
- **Attach.** `vactr lsp --session <url>` connects to a running
  `serve` socket, subscribes to diagnostics, and merges runtime `diag`s
  into the matching document's diagnostics. Standalone, it serves
  static diagnostics only (14.3).
- **Smoke test.** `tests/lsp_smoke.rs`, under
  `#![cfg(feature = "lsp")]`, starts `CARGO_BIN_EXE_vactr lsp` over
  pipes with Content-Length framing. It sends `initialize`, `didOpen`
  of a lang-reference spec block, and a `hover` over a bound name, and
  asserts one `publishDiagnostics` for the URI and a hover whose
  contents contain the type.

#### 14.5.12 Codes, waves and verification

**New codes (closed list), added by CONTRACTS in one edit.**
- `DiagCode`, directive lint, all (w): `unknown-directive-site`,
  `unknown-parameter`, `unknown-label`, `duplicate-label`,
  `ambiguous-selector`, `cc-out-of-range`, `reserved-key`.
- `DiagCode`, packages: `package-not-locked`, `package-not-fetched`
  (w), `package-integrity`, `package-load-failed`, `package-resolve`
  (from `vactr get`: an unresolvable version or a store or network
  error).
- `FailCode`: `capture-pending`, `beyond-capability`.
- Protocol `protocol-error` codes and `stale-binding` reasons are
  protocol strings (14.5.6), not `DiagCode`s.

**Waves.** Plans are `impl-plans/active/vactr-session-<wave>.md`.
Each lists its own plan file in its manifest `writePaths`. Only FINAL
edits `vactr-core.md` and `impl-plans/README.md`, and FINAL never
edits the dispatch manifest.

| Wave | Content | Depends on |
|------|---------|------------|
| CONTRACTS | Cargo deps and features plus version pinning and audit; the `src/lib.rs` declarations with stub `mod.rs` files; `session/changes.rs` with its tests and the seed `session/mod.rs` (`pub mod changes;` only); the codes; the `Sound::Buffer`/`SampleBuf` shape with minimal arms; `StagedEffect::{Capture, Render}` shapes (the runtime ignores them until ANALYSIS); the `host/caps.rs` default methods and the `host/native/tap.rs` stub (declared in `host/native/mod.rs`); `value/sample.rs` (declared in `value/mod.rs`); the `ns/eval_doc.rs` stub (declared in `ns/mod.rs`); the `compile/compiler.rs` split into `compile/compiler.rs` + `compile/names.rs` (declared in `compile/mod.rs`) plus console-register resolution; `Namespace` console slots | — |
| PKG | `pkg/`, `ns/pkg.rs`, `host/native/loader.rs` | CONTRACTS |
| DIRECTIVES | `directives/`, `tests/directive_fixtures.rs`, `tests/fixtures/directives/vocabulary.toml` | CONTRACTS |
| ANALYSIS | the 14.5.9 files, plus repinning design-music ordinal 2 in the spec manifest | CONTRACTS |
| SESSION | `session/` (every file except `changes.rs`), `ns/eval_doc.rs`: pipeline, publication, authority, codec, console, REPL loop, and the protocol, publication, stale-write, package-through-session and recording-host tests | PKG, DIRECTIVES, ANALYSIS |
| CLI | `src/main.rs`, `cli/` (including `cli/ws.rs`), `tests/cli.rs` (the binary with `--host noop`: `run --cycles`, `repl` over piped stdin, `get --store dir:` in a temp `VACTR_HOME`, and `serve` with a token-checked client round trip) | SESSION |
| LSP | `lsp/`, `tests/lsp_smoke.rs` | SESSION |
| FINAL | reclassify lang-reference ordinal 5 (evaluated through `Session`, `tests/support/eval.rs`); core-plan TASK-009 checkboxes, progress log and the pending audible gate; README; archive | CLI, LSP |

The waves run in this order: CONTRACTS; then PKG + DIRECTIVES +
ANALYSIS; then SESSION; then CLI + LSP; then FINAL.

**Verification.** The 6.5.7 evidence rule applies to every wave, with
`<plan>` = `ss-<wave>`. The checks are:
- `build`, `build-lsp` (`cargo build --features lsp`), `clippy`
  (`--all-targets -- -D warnings`), `clippy-lsp` (the same with
  `--features lsp`), `fmt` (`--check`), `nextest`, `test` (plain
  `CARGO_TERM_QUIET=true cargo test`), `wasm32`, `wasm32-hostwasm`,
  `linecount` (the largest `.rs` is under 800) and `tree-wasm32` (none
  of the gated crates appears);
- CONTRACTS also runs `audit`;
- LSP also runs `lsp-smoke` (`cargo test --features lsp --test
  lsp_smoke`).

The audible REPL gate stays a manual step, recorded as pending user
confirmation. Its automated proxy is the SESSION recording-host test:
a REPL-bound pattern reaches the audio host as events. Rollback is a
`git revert` of the single implementation commit; nothing migrates.

## 15. Editor: Tauri, Web, Wasm — and What Performance Needs

One TypeScript frontend (`editor/`, Vite), delivered two ways from the same
code. The historical CodeMirror visible surface below is superseded by
15.3 for the canvas-editor workflow; its state and revision machinery is
retained where compatible:

| Delivery | Core | Audio |
|----------|------|-------|
| Browser / PWA (first) | Wasm core on the main thread | second Wasm instance inside the AudioWorklet (section 16) |
| Tauri desktop | same web frontend in the webview, same Wasm core (v1 INTERIM delivery) | same worklet path; the native tier (native core in the Tauri app, CoreAudio/CoreMIDI) is the Decided SECOND delivery milestone (1, 12.7), outside this plan — the session-socket seam is its upgrade path |

Running the identical Wasm runtime inside Tauri keeps one code path
(web-first, decided); the Tauri shell adds file open/save of `.vact`,
MIDI access fallback, and packaging. iPad Safari is covered by the PWA.

Editor features (the TidalCycles/Strudel/algorave set), each mapping to
runtime machinery above:

| Feature | Backed by |
|---------|-----------|
| Eval form / eval line / eval-flash (Ctrl+Enter etc.) | `eval` message; span flash |
| Inline diagnostics while typing | reader recovery + checker (7) via wasm call |
| Runtime error on the originating line, with slot + beat | `diag` messages (8.4, 14.4) |
| **Highlight of currently sounding steps/events** | `playing` telemetry with source spans (10.2, 11.6) |
| Per-slot activity, level meters, orbit view | `levels`, telemetry window |
| `hush` / panic button, per-slot stop/mute | `hush`/`stop` messages, slot table |
| Tempo display, cycle/beat position | `tempo` message; clock (11.1) |
| Sample bank browser with preview, keyword completion | host manifest (7), `SampleLoader` |
| Pattern combinators and controls (fast, every, euclid, degrade-by, jux, off, stack/cat, scale/chord/arp, effects room/size/delay/crush/shape/vowel, …) | pattern engine (10), control handoff (11.4) |
| Euclidean + probabilistic sequencing, seeded randomness | `Euclid`, `maybe`/`choose`/`degrade-by`, seeded RNG (10.3) |
| Signals (`sine`, `rand`, `perlin`, `range`, `segment`, fft/amp) | 10.5, 12.3 |
| One-shots `once`, scheduled `at` | 11.2 |
| MIDI/OSC sinks (`midi 1`, `osc "/addr"`) | 11.4, MidiHost/OscHost |
| Right-pane slider panel over EVERY enumerated numeric site (pattern/control literals, top-level let/var, inst defaults), source-edit AND overlay modes | controller binding (13), Decided Editor Requirements |
| DAW-style parameter editors per builtin meaning (EQ curve + live spectrum, filter response, dynamics transfer, envelope, delay taps, sampler/wavetable/granular views, LFO shape, XY pad, euclid ring, probability dial), all handles = slider sites, all MIDI-learnable | 13.5 EditorDecl/ParamMeta |
| Step grid and piano roll as DISPLAYS of what is sounding (never sequence editors) | 13.5, telemetry 11.6 |
| `#@` directive comments: control-panel setup, labels, file-level MIDI defaults, learned-CC write-back into the comment | 13.5 DirectiveTable/LabelRegistry (authority adjudicated; vocabulary PROPOSED) |
| Sliders / MIDI-learn / mouse-drag on literals and bindings | controller binding (13) |
| MIDI input: `cc` signal, note input (`midi-notes`), MIDI learn | 11.7, 10.1, 10.5 |
| MIDI clock/transport sync in and out (Link planned, diagnostic in v1) | 11.1, 11.7 |
| Analyzer meters and scopes (level, spectrum, spectrogram, oscilloscope, …) | 12.5 analysis cells over telemetry |
| Package import (`vactr get`, proxy in the browser), package diagnostics | 5.7 |
| Visual output panes o0..o3 | visual pipeline (9), WebGL2 RenderHost in the TS shell |

The editor may show visual feedback about music (steps, events,
levels); it does not couple the music and visual LANGUAGES — that stays
deferred.

### 15.1 Editor implementation decisions (TASK-010, 2026-09-26)

This section fixes the choices that sections 9, 11.7, 12.5, 13, 13.5,
14.4, 15, 16 and 17 leave open for issue #5 (the `editor/` app, the
browser delivery and the Tauri shell). It follows the shape of 12.8 and
14.5 and changes no Decided behavior. The wire additions and the
browser ABI are in `design-docs/specs/command.md`; this section holds
the rules. Open author questions are in
`design-docs/user-qa/pending-editor-questions.md` (E1-E5). The
implementation follows each recommendation until it is answered.

#### 15.1.1 Scope boundary

In scope: every TASK-010 deliverable and completion criterion of
`impl-plans/active/vactr-core.md`, plus the two items TASK-009 moved
here, with the dispositions below.

- **One frontend, two transports.** The editor speaks Session Protocol
  v1 only. The browser tier runs a `Session` inside wasm #1 and passes
  the same JSON text through the raw ABI (15.1.2 G1). The native tier
  connects to `vactr serve` over the WebSocket. No UI component knows
  which transport it has, except the four tier-dependent features
  listed in 15.1.4.
- **Rust changes only where the protocol cannot express a criterion.**
  The contract gaps G1-G6 (15.1.2) are the complete list. Each is
  additive: `v` stays 1, every new field is optional, and old clients
  ignore it. No Decided behavior changes, and the native CLI, REPL and
  LSP behave exactly as before.
- **Browser package store: done here** (moved from TASK-009, 14.5.1).
  `fetch()` is the proxy transport, driven by a need-URL loop around
  the existing core `pkg::get_all`. OPFS persists the digest-pinned
  proxy responses and the lock (15.1.10).
- **Browser self-analysis taps: NOT done here** (moved from TASK-009,
  recommendation E2). `scope`/`spectrum`/`capture` on a live source
  still fail in the browser with "not available on this host". No
  TASK-010 criterion needs them, and they would add rendering-thread
  record traffic under the 16.1 bounded-work rule. The editor's meters
  and scopes do not use taps. They render from the analyzer cells
  (12.5) that already reach the runtime on both tiers (G4).
- **No UI framework.** Plain DOM components plus CodeMirror 6. Each
  component is a `mount(root, deps)` function returning a handle.
  *Superseded (author, 2026-09-26) by 15.2: the panels move to Solid.js.*
- **Out of scope** (each is a residual risk, none is a criterion):
  - native-tier visuals (the native session keeps `NoopRender`, 9.4);
  - browser MIDI output and clock-out (E2);
  - `use-fps`/`use-canvas`, which reach no `RenderHost` method today;
  - live gain-reduction metering, because no GR cell exists;
  - the MIDI access fallback in Tauri (15 lists it). WKWebView has no
    WebMIDI, so Tauri reports "not available on this host", the Decided
    host-tier rule for a missing capability.

#### 15.1.2 Rust contract additions (G1-G6)

One Rust plan pair owns every Rust file below (15.1.12). All wire
shapes are in `command.md`.

- **G1: browser Session over the raw ABI.** A new
  `src/host/wasm/session_half.rs` (declared in `host/wasm/mod.rs`,
  compiled under `all(target_arch = "wasm32", feature = "host-wasm")`)
  builds `Session::new` with:
  - `CapabilitySet::browser()`;
  - `RuntimeConfig { tier: Tier::Browser(WasmCellPort) }`;
  - the shared `InstRegistry`, `WasmAudioHost`, `WasmSamples`, the new
    `WasmRenderHost` and `WasmMidiIn`;
  - the in-memory package cache of G6;
  - `PersistenceMode::Directive`.

  Its exports:
  - `session_init(sample_rate, arena_bytes)`;
  - `session_apply(ptr, len)`: one UTF-8 envelope, run through
    `Session::apply_text` on connection 1;
  - `session_tick(now)`, `session_inbox(ptr, len)` and
    `session_sample_put(...)`, the session twins of `tick`, `inbox`
    and `sample_put`;
  - `session_check(ptr, len)`: static analysis of the document text via
    `session::eval::analyze` plus the directive lint. It never
    executes;
  - `session_frame(now)`: G5;
  - `session_midi_in(ptr, len, time)`: raw MIDI bytes at an audio-clock
    time, buffered for `MidiInHost::poll`, which feeds `cc`, note
    input and clock sync (11.7);
  - `pkg_resolve` and `pkg_supply`: G6.

  Outputs go to the existing outbox under the new tags `TAG_SESSION`
  (0x71, one server envelope as JSON text), `TAG_RENDER` (0x72, a
  `set_program` or `set_uniforms` record as JSON) and `TAG_PKG` (0x73,
  a package-driver reply as JSON). These tags are free: wire uses
  0x01-0x1A and 0x40-0x48, abi uses 0x60, 0x61 and 0x70. A page
  initializes EITHER `main_init` (the dev harness, unchanged) or
  `session_init` (the editor). `main_half.rs` is NOT edited, because
  the TASK-008 harness evidence cannot be re-run headless in every
  sandbox. The session half carries its own copy of the small
  worklet-record decoding (the same `TAG_FAULT`, `TAG_SIGS` and
  `HostMsg` cases).
- **G2: editor metadata on the wire.** `manifest` gains `editors`: every
  `dsp::meta::all()` entry, plus a fixed PATTERN-FUNCTION table in a new
  `src/session/editors.rs`:
  - `euclid` -> `euclid-ring` (hits, steps, rotation);
  - `maybe` and `degrade-by` -> `probability-dial`;
  - `hold`, `fast` and `slow` -> `length-handle`;
  - `sine`, `saw`, `tri`, `square`, `rand` and `perlin` -> `lfo-shape`.

  Pattern-function params carry no `ctl`, because they have no
  control-table row. The table lives in the core, not in the editor,
  so the editor, LSP and directives share one source (13.5, "every
  builtin declares"). Ranges and editor kinds are still never
  persisted.
- **G3: site call identity.** A `site` gains
  `call: {name, head, ordinal, arg, param?}`:
  - the call is the nearest enclosing symbol-headed call that the
    literal is an argument of, directly or inside a list or pattern
    argument (so `n [0 3 5]` gives name `n` for all three literals);
  - `ordinal` counts same-named call sites in the top-level form
    (1-based, as in 13.5);
  - `arg` is the 0-based argument index;
  - `param` is the named-argument keyword, else the declared parameter
    at that position (EditorDecl order or the definition header, the
    same rule as `directives/attach.rs` `CallSite::params`). It is
    absent when unknown.

  Heads in `attach.rs` `NOT_SITES` are never calls. This lets
  parameter editors, the sampler editor and CC routing find their sites
  without the editor parsing the language.

  Ownership: `directives/attach.rs` `CallSite` gains its argument spans
  (and named-argument keywords). `directives/mod.rs` returns the
  document's call sites alongside the table. `session/publish.rs`
  `site_wire` maps each site to its innermost call from that list.
- **G4: analysis and clock telemetry.**
  - `levels[0]` (`:master`) gains `bands`, the 8 host FFT bands already
    in `HostSigs`.
  - `levels` gains `analyzers`: for every `EffectKind::Analyzer` unit
    of the installed bus graph, its bus, kind, first cell `id` and
    current values `cells(kind)`, read from the runtime `InputCells`
    that `HostMsg::AnalysisCell` already fills on both tiers.

  Publication stays at most 10 per second and subscribers only. An
  analyzer whose `id` is not a constant is left out.

  `tempo` gains `clock: {source: "internal"|"midi", locked?: bool}`,
  from `Runtime::clock().source()` and the MIDI slave state
  (`is_lost`). A clock change also triggers a `tempo` message. This
  serves the transport bar's MIDI clock status on both tiers.
- **G5: per-frame uniforms and render records.**
  - `Runtime::activate` keeps the `UniformPlan` that `compile_tex`
    returns, which it discards today, per output.
  - A new `sched/render.rs` adds `Runtime::render_frame(ev, now)`. It
    resolves every active plan (9.3) and calls
    `RenderHost::set_uniforms`.
  - `Session::render_frame(now)` forwards to it.
  - `WasmRenderHost` encodes `set_program` and `set_uniforms` as the
    `TAG_RENDER` records of `command.md` (`op` `program` or
    `uniforms`).
  - `hush`/`stop` of a visual slot already sends the empty program
    (`sched/control.rs`). The TS host draws that as black.
- **G6: browser packages.**
  - A new core `src/pkg/mem_cache.rs` implements `CacheBackend` in
    memory, with the same staging and atomic-publish contract.
  - `Session` gains one additive method that lends the package driver
    the session's own cache backend (`&mut dyn CacheBackend`) and
    installs the resulting lock. The session still owns the cache, so
    the next eval loads from exactly what the driver published.

  The package driver (15.1.10) runs `pkg::get_all` (root requirements
  as a `PkgManifest`) or, for a restore, `fetch_and_publish` against
  the persisted lock, over `ProxyStore<Prefetched>`. `Prefetched` is a
  `ProxyTransport` whose `get` answers from supplied URL bodies and
  records the first missing URL. The driver then returns `need` for
  that URL, and the partial run has already discarded its staging
  (14.5.7). Success installs the lock into the session.

Every Rust file stays under 800 lines (14.5.3). `session/session.rs`
(703) splits if the additions push it to 800. Rust tests cover G2-G6
natively under `src/session/tests/`, `src/sched/tests/` and
`src/pkg/tests/`. G1 is wasm-only glue. Its owner tests it with a
real-wasm ABI smoke test, and FINAL's criterion tests exercise it end
to end (15.1.12).

#### 15.1.3 Tooling, dependencies and layout

- **Toolchain.**
  - Node is taken from the environment. `package.json` declares
    `engines.node >= 20`.
  - mise does not pin node (E5), because a mise node install is a
    download from outside the npm registry.
  - `npm ci` installs from the committed `editor/package-lock.json`.
- **Dependencies** (exact versions, pinned once by the scaffold plan,
  which runs `npm audit` for its evidence):
  - runtime: `@codemirror/{state,view,language,commands,lint}`,
    `@lezer/highlight`, `@tauri-apps/api`,
    `@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-fs`;
  - dev: `typescript`, `vite`, `vitest`, `jsdom`.

  No other package is added by any later plan. The scaffold plan is the
  only writer of `package.json` and the lockfile.
- **Scripts.**
  - `check`: `tsc --noEmit`.
  - `test`: `vitest run`.
  - `build`: `vite build`.
  - `dev`: `vite`, which is not part of verification.
- **Wasm artifact.**
  - The editor uses `$VACTR_WASM`, else
    `../target/wasm32-unknown-unknown/debug/vactr.wasm`, the output
    of the mandatory `--features host-wasm` build.
  - `vite build` copies it to `dist/vactr.wasm`, and
    `worklet/processor.js` to `dist/worklet/`.
  - The build FAILS if the file is missing, and the real-wasm tests fail
    (never skip) if it is missing or lacks the `session_init` export.
    The build's own export check runs when `VACTR_REQUIRE_SESSION_ABI=1`,
    which the plans that land or join G1 set (the export arrives in a
    later wave than the scaffold).
  - `vite build` honors `--outDir`, so parallel plans build into their
    own directory and never race on the shared `dist/` (15.1.12).
  - Because the default-feature wasm32 build writes the same path, the
    verification order is fixed: wasm32 default, then wasm32
    host-wasm, then the editor steps (15.1.12).
- **Worklet glue.** `editor/worklet/host.js` gains two additive options:
  - `init: 'main' | 'session'`: the default `'main'` keeps the harness
    exactly as before, and `'session'` calls the `session_*` exports;
  - `onRecord(tag, bytes)`: receives 0x71-0x73 records, which are
    never posted to the worklet.

  A typed declaration `host.d.ts` accompanies it. `processor.js` is
  unchanged.
- **Tests.** vitest with the `jsdom` environment by default. The
  real-wasm files declare `@vitest-environment node`. CodeMirror runs
  under jsdom. WebGL2, `MIDIAccess`, canvas 2D, `AudioContext`, OPFS
  and `fetch` are recording mocks under `editor/test/support/`, each
  owned by the plan whose code uses it.
- **Git hygiene.** A new `editor/.gitignore` covers `dist/`,
  `src-tauri/target/` and `src-tauri/gen/`. The root `.gitignore`
  already covers `node_modules`. It also ignores `*.zip`, so fixture
  zips are built inside the tests, never committed.
- **Layout.** `editor/src/<area>/` with the areas:
  - `app`: shell, layout, the `mount` registry;
  - `protocol`: types, envelope, client, transports;
  - `code`: CodeMirror mode, diagnostics, eval, highlight, reconcile,
    transport bar, sample browser;
  - `bind`: sites, keys, slider panel, drag, write-back, directives
    panel, persistence, save;
  - `params`: parameter editors, grid, roll;
  - `visual`: render host, panes, meters and scopes;
  - `midi`;
  - `pkg`;
  - `platform`: file access.

  `app/main.ts` imports each area's `mount.ts`. The scaffold creates
  every `mount.ts` as a stub and the area owner fills it (the 14.5.3
  seed-then-fill rule). TS files also stay under 800 lines.

#### 15.1.4 Protocol client

- **Transports.** `Transport { send(text), onText(cb), close() }` with
  three implementations:
  - `WasmTransport`: `session_apply` in, `TAG_SESSION` records out,
    ticked by host.js;
  - `SocketTransport`: `ws://127.0.0.1:<port>/session?token=<hex>`,
    where the token is pasted by the user and never stored;
  - `RecordingTransport`: tests; it records every client envelope and
    replays scripted server envelopes.

  The client owns `seq`, matches replies by `re`, and dispatches
  broadcasts to a store (15.1.6 "Reactive displays").
- **Revisions and epochs per document.**
  - `doc_revision` starts at 1 on open and increments on every
    CodeMirror transaction that changes the document.
  - `edit_epoch` increments on EVERY local edit, synchronously, before
    any debounce (14.4).
  - `doc-changed` is debounced at 200 ms, and it is FLUSHED before any
    `set-tweak`, `set-var`, `learn` or `eval` for that document (14.4
    rule 1).
  - Every write is stamped with the epoch current when its input was
    produced.
- **Offsets.** The protocol uses UTF-8 byte offsets and CodeMirror uses
  UTF-16 units. The client converts through a per-revision UTF-8 index,
  in both directions.
  - Changes are `{from, to, insert_len}` in base-revision bytes.
  - Dirty spans are the new-revision byte ranges of the inserted text,
    plus a zero-length span at each pure deletion.
  - Tests use non-ASCII text.
- **Stale handling.**
  - `stale-form-gen`: the value is kept and re-sent latest-wins to the
    re-keyed site once fresh sites arrive.
  - `edit-invalidated` and `unreconciled-edit`: the slider shows STALE
    until an eval brings fresh sites.
  - `superseded-definition`: the target is dropped.
- **Rate.** Controller writes are limited per target to one per 16 ms,
  latest-wins. The session coalesces further per tick (14.5.6).
- **Tier-dependent features** (the only places the UI asks the tier):
  - visual panes and typing-time static diagnostics (`session_check`):
    browser only. The native tier shows static diagnostics from
    `eval-result`.
  - package import: browser only. The native tier shows the
    `vactr get <path>` command to run, because a running session
    never fetches (14.5.1).
  - highlight timing: the browser schedules on
    `AudioContext.currentTime`, which is the session's clock. The
    native tier anchors host times at batch receipt, best effort and
    not asserted.
  - sample waveform previews: browser only.

#### 15.1.5 Code surface

- **`.vact` mode.** A CodeMirror `StreamLanguage` tokenizer for
  highlighting only: comments, `#@` directives, keywords (`:kw`),
  numbers, strings, path/url literals, and the definition heads. It is
  never used for binding decisions. Diagnostics come from
  `session_check` (browser, debounced 300 ms), from `eval-result`, and
  from runtime `diag`, shown with slot and beat. A `diag` `clear` for a
  slot removes that slot's runtime markers.
- **Eval.**
  - `Mod-Enter` evaluates the form at the cursor. Its span runs from
    the nearest line at or above the cursor that starts at column 0
    with a character other than space, `#` or `>`, through the line
    before the next such line, minus trailing blank lines. The session
    then selects the forms inside it (14.5.4).
  - `Mod-Shift-Enter` evaluates the whole document (no span).
  - `Mod-.` sends `hush`.
  - `eval.code` is always the full text (14.5.6).
  - The eval span flashes for 200 ms, and forms with a `failure` flash
    the error color.
- **Highlighting.**
  - A `playing` event with `src` is scheduled on the tier clock
    (`Clock.now()`, with `MockClock` in tests).
  - The decoration is active over `[time, time + dur_seconds)`, where
    `dur_seconds` comes from `dur` and the latest `tempo`.
  - Its span is mapped from the event's own `doc_revision` to the
    current one through a bounded history of CodeMirror `ChangeSet`s
    (the last 256 revisions).
  - A span touched by a change, or older than the history, is dropped.
  - Events without `src` highlight nothing. Events whose list has no
    element provenance already carry the binding span (5.4).
- **Transport bar.**
  - Tempo and cycle/beat come from `tempo`, extrapolated locally
    between messages.
  - MIDI clock status comes from `tempo.clock` (`internal`,
    `midi locked`, `midi lost`).
  - hush/panic sends `hush`. The protocol has no separate panic, and
    the session's hush already releases with panic.
  - The per-slot list is built from `playing` and `eval-result`. It
    shows an activity light from telemetry, and mute (`stop`, the only
    per-slot message).
  - Per-slot levels are not available (E3). The master meter comes
    from `levels`.
- **Sample bank browser.**
  - It lists `manifest.sounds`.
  - In the browser, a `SampleLibrary` loads a user-configured sample
    map (`{"<bank>": ["<url>", ...]}`), decodes entries with
    `decodeAudioData`, and hands them over as `bank:index` through
    `session_sample_put`. Admission and the arena are 16.1's.
  - Per-entry waveform previews are drawn from the decoded frames.

#### 15.1.6 Binding UI

- **Sites and keys.**
  - The slider panel lists every `site` of the latest `eval-result`,
    updated by `bindings.sites`, grouped by `origin` (pattern literal,
    binding, inst default).
  - Binding identity follows 13 and 13.5. A site with `key` is keyed by
    its `BindingKey` spelling.
  - Any other site is keyed by its tracked span, mapped through the
    change history. It is re-keyed to the fresh `TweakId` whose span
    matches when a new site table arrives. When nothing matches it
    shows as unbound, and when the mapping was touched it shows as
    STALE.
  - Duplicate literals are distinct sites and bind independently.
- **Two modes per slider** (13, both Decided).
  - OVERLAY sends `set-tweak {id, form_gen, value, edit_epoch}`. The
    text is untouched, and the overlay value is rendered beside the
    literal.
  - SOURCE-EDIT and overlay COMMIT both do a VALIDATED TEXT EDIT, then
    an `eval` of the owning form's span:
    - the edit applies only if the current text at the mapped span
      still spells the last-known literal; otherwise it declines with
      a notice;
    - the literal is formatted as an integer when the original literal
      had no `.`, and trimmed to the ParamMeta step otherwise;
    - at most one eval is in flight per form, latest-wins.
  - Mouse drag on a literal that is a site, and a learned CC, are front
    ends to the same two paths. The drag scale is the ParamMeta range
    and curve when `site.call.param` has one, else relative to the
    magnitude.
- **Directive control panel.** It renders `eval-result.directives`
  (entries, labels, bindings). Directive lint diagnostics
  (`unknown-label` and the others) render as editor diagnostics. Panel
  membership is edited only in the text. The v1 UI has no
  add-to-panel action (no criterion needs one).
- **CC routing.** Incoming CC `(ch, cc)` resolves to a binding:
  - Directive mode: through the directive table's `bindings`, matched
    to a site by `key`, else by span containment plus
    `site.call.param`;
  - ExternalFile mode: through the editor-side set.

  The binding's slider mode then decides the path. Keys are the full
  `BindingKey`, so `hats.lpf`, `hats.hpf`, `hats.lpf.1` and
  `hats.lpf.2` never cross-talk.
- **Learn.**
  - Directive mode sends `learn {binding: key | tweak id, cc, ch}`,
    then applies the returned `directive-edit` only after checking
    `expected` against the current text at the mapped span, then sends
    `doc-changed`.
  - In ExternalFile mode the editor updates its own set and sends no
    `learn` (E4).
- **Persistence and saving (MODE-SCOPED, 13).**
  - DIRECTIVE mode (the default, and the session's mode on both tiers):
    the saved `.vact` is the buffer text, `#@` comments with learned
    CCs included, and nothing else is added.
  - EXTERNALFILE mode (E4): the editor keeps a binding set
    `{key, panel, midi?, overlay?}` and writes `<doc>.bindings.json` in
    the 14.5.8 format (`{"v": 1, "bindings": [...]}`, keys spelled
    `label.site.n.param`, overlays kept).
    - It never inserts or edits `#@` text, so the saved `.vact` gets no
      binding artifact from the editor.
    - It never strips `#@` lines the user wrote, so "source untouched"
      holds.
    - Switching into ExternalFile copies the current panel (directive
      bindings plus learned mappings) into the set. The panel is
      preserved and no text changes.
  - In BOTH modes an overlay value reaches the text only through an
    explicit commit.
  - Save goes through `platform/files.ts`: File System Access or a
    download in the browser, dialog-scoped fs in Tauri, and an
    in-memory implementation in tests.
- **Reactive displays.**
  - A store applies each `bindings` batch ATOMICALLY: `changed` values,
    `sites` re-keying, and `states` badges (`failed` with its
    diagnostic and restored value, `blocked` with `blocked_on`, `ok`
    clearing).
  - It then runs ONE repaint of exactly the components subscribed to
    an affected name, site or slot. Unrelated components do not
    repaint, and tests count renders.
  - Nothing is rendered from anything but completed batches, so a
    provisional value can never appear (14.5.5).

#### 15.1.7 Parameter editors and displays

- **Opening.**
  - A call site is chosen from the same enumeration: the sites grouped
    by `site.call` (name, head, ordinal) in one top-level form.
  - The editor kind comes from `manifest.editors[name]`. A name with no
    entry, or an unknown kind, gets the `scalar` editor (sliders).
  - Each handle is bound to the site whose `call.param` (else `arg`)
    matches. A missing site means a disabled handle, never a text
    insertion.
- **Handles are sliders.** Every handle calls the same site-write
  function as the slider (both modes) and is MIDI-learnable through the
  same learn path. The criterion test asserts that a drag and the
  equivalent slider move record the identical `set-tweak` (id,
  `form_gen`, value).
- **Kinds.**
  - `eq-curve`: bands, with the live spectrum drawn behind them. The
    spectrum is a `spectrum` analyzer's cells on the site's bus when
    one exists (G4), else the `:master` `bands`.
  - `filter-response`: a response curve.
  - `dynamics-transfer`: a transfer curve (gain-reduction metering is
    out of scope).
  - `envelope-shape`: stages.
  - `delay-taps`: beat-aligned taps.
  - `sampler-wave`, `wavetable-frames` and `granular-region`: views.
  - `lfo-shape`: the shape, from the signal call's numeric sites.
  - `stereo-field`: a pan field.
  - `xy-pad`: the user picks any two sites.
  - `euclid-ring`: hits, steps and rotation handles.
  - `probability-dial` and `length-handle`.
- **Sampler waveform** (13.5).
  - The waveform comes from `SampleLibrary` frames for the call's bank
    and `n` (browser tier).
  - Start/end/loop handles are the `begin`/`end`/`loop` sites
    (`call.name`) in the same top-level form.
  - `slice n`, `chop n` and `striate n` counts draw as grid overlays.
  - Manual slice markers are the numeric sites of the `slice` point
    list, and each drag writes through the standard path.
  - Clicking a slice writes the clicked index into the SELECTED,
    EXISTING index literal, through the same validated write. The
    selection is the site under the editor cursor or the one last
    focused in the panel. With no selection the click does nothing and
    shows a hint. The editor never adds, removes or reorders steps.
  - The `n` site opens the sample browser at that bank.
- **Step grid and piano roll are DISPLAYS.**
  - Both are rendered from `playing` events per slot, with steps
    placed by beat within the cycle.
  - The roll's pitch is read, for display only, from the source text
    at the event's mapped `src` span: a note keyword or a number. Other
    text goes to an unpitched lane.
  - Their modules import neither the protocol client nor the write-back
    module. The structural test scans their imports and then fires
    pointer and keyboard events, asserting that no client envelope is
    recorded and the document is unchanged.

#### 15.1.8 Visual panes and analyzer displays

- **WebGL2 RenderHost** (9.2-9.4, browser and Tauri).
  - It consumes `TAG_RENDER` records.
  - Each output `o0..o3` owns two framebuffers (ping-pong). `src oN`
    samples the previous frame.
  - `render oN` selects the displayed pane, and `render` tiles all
    four.
  - The builtin uniforms `time` (audio clock seconds) and `resolution`
    are set by the host. The named uniforms come from `set_uniforms`,
    and `session_frame(now)` runs once per `requestAnimationFrame`.
  - `TextAsset` text is rasterized with canvas 2D into a texture bound
    at the asset id's sampler.
  - A GLSL compile or link failure keeps the previous program
    rendering and reports a host diagnostic. A language-level broken
    chain never reaches the host: its slot keeps the previous binding,
    and the eval shows the diagnostic.
  - Activation at the cycle boundary is the runtime's
    (`Runtime::activate`). The host draws whatever program it last
    received.
- **Meters and scopes** come from `levels` (G4):
  - a master level meter (`rms`) and an 8-band spectrum (`bands`);
  - one display per published analyzer, chosen by kind: `level`,
    `spectrum`, `spectrogram` (from the ring cells), `oscilloscope`
    (from the ring cells), `pitch-meter` and `stereo-meter`.

  The editor never opens an audio input: there is no `getUserMedia`
  call anywhere (the Decided "never monitors the input device unless
  asked"; v1 has no ask path).

#### 15.1.9 MIDI

- **Access.** `navigator.requestMIDIAccess({sysex: false})` is called
  only on a user action. Where it is absent (Tauri WKWebView), the
  editor shows "not available on this host".
- **Device picker.** It chooses the inputs whose messages are:
  1. used for learn and CC routing (15.1.6), on both tiers;
  2. forwarded to the session as `cc`, note input and clock (browser
     tier only), through `session_midi_in`. Timestamps are converted
     to the audio clock with `AudioContext.getOutputTimestamp()`.

  On the native tier, the session's own midir input serves the
  language.
- **Learn.** The next CC (with its channel) after "learn" on a slider or
  a handle becomes the binding's mapping (15.1.6).
- **Clock status.** Shown from `tempo.clock`.

#### 15.1.10 Packages in the browser

- **UI.**
  - It lists unresolved imports from `package-not-locked` and
    `package-not-fetched` diagnostics, and the proxy URL, which is
    user-configured and stored in `localStorage`. There is no default
    proxy.
  - "Import" runs the driver loop:
    1. `pkg_resolve({proxy, requirements})` returns `need <url>`;
    2. `fetch(url)`;
    3. `pkg_supply(url, status, bytes)`;
    4. repeat until `done` (lock text, resolved entries) or `error`
       (code and message, shown as a package diagnostic).
  - Load diagnostics then arrive with the next eval, started by the
    user. The UI never auto-evaluates.
  - Requests go only to the configured proxy. The proxy must allow
    CORS.
- **OPFS.**
  - The root requirements, the lock text and every successful proxy
    response body, keyed by URL, are stored under
    `vactr-pkg/`.
  - On load, the editor supplies the stored bodies and runs
    `pkg_resolve({proxy, lock})`. That restore re-validates and
    compares every digest with the lock.
  - A mismatch is `package-integrity`, and the entry is deleted from
    OPFS.
  - Content reaches the session cache only through validation, digest
    and staged publication (17). OPFS holds raw bytes, never a trusted
    tree.
  - Without OPFS, packages are memory-only and a hint is shown.
- **Native tier.** The UI shows the diagnostics and the exact
  `vactr get <path>` command. It never fetches (14.5.1).

#### 15.1.11 Tauri shell

- **Crate.** `editor/src-tauri/` holds a standalone crate
  (`vactr-editor`, `publish = false`) with its own empty `[workspace]`
  table, `Cargo.lock` and `target/`. The root crate has no
  `[workspace]` and never builds it. Root `cargo build`, `clippy`,
  `fmt` and `nextest` are unaffected.
- **Dependencies.**
  - `tauri` 2, `tauri-build` 2, `tauri-plugin-dialog` 2 and
    `tauri-plugin-fs` 2 only.
  - Versions follow the 12.8.10 policy on Rust 1.83: the highest whose
    tree builds, with `cargo update --precise` where needed.
  - Fetching them from crates.io is E1. Without that access, the
    `cargo check` gate is BLOCKED, which is not passing.
- **Frontend.** `frontendDist` is `../dist`, the identical Vite build.
  `cargo check` therefore runs after `npm run build`, because
  `generate_context!` embeds `dist/`. A minimal icon is committed if
  tauri-codegen requires one.
- **Allowlist** (17): capability `default` with `core:default`,
  `dialog:allow-open`, `dialog:allow-save`, `fs:allow-read-text-file`
  and `fs:allow-write-text-file`.
  - There is no static fs scope: only paths the user picked in a dialog
    are allowed (`.vact` and `.bindings.json` filters).
  - There is no shell plugin and no custom command.
  - The CSP is `default-src 'self'; script-src 'self'
    'wasm-unsafe-eval'; connect-src 'self' ws://127.0.0.1:* https:;
    img-src 'self' data: blob:; worker-src 'self' blob:`.
- **Evidence.** `cargo tauri build` and running the app are manual,
  pending user confirmation.

#### 15.1.12 Tests, waves and verification

**Criteria to tests.** File names are indicative; the plans enumerate
them.
- **Criterion 1: highlight within one lookahead window.** Mock-clock
  highlight tests (browser and edit mapping). The real-wasm test runs
  `eval` + `session_tick` at mock times and asserts `playing` with
  `src`/`doc_revision` within the lookahead. Hearing the audio is
  manual, pending user confirmation.
- **Criterion 2: direct/reeval/manual and the two modes.** Slider tests
  on a RecordingTransport: overlay `set-tweak` with the text unchanged
  until commit; source edit with verified text plus form eval. The
  real-wasm test covers a `direct` and a `reeval` site through
  `bindings`.
- **Criterion 3: site kinds, learn, meters, package UI.** Panel
  grouping over all three origins; learn to slider; meter rendering
  from scripted `levels` with `analyzers`; the package UI against a
  mocked proxy serving an in-test zip, plus a real-wasm driver test
  with an integrity failure surfacing as a diagnostic.
- **Criterion 4: parameter editors and displays.** `peq` opens
  `eq-curve` with the spectrum; `env-adsr` opens `envelope-shape`;
  `euclid` opens `euclid-ring`. Handle equals slider on the recording
  transport. Learnable. Grid and roll pass the structural test.
- **Criterion 5: directive round trip.** Spec directives give the
  panel; learn applies `directive-edit`; a dangling label gives a
  diagnostic; switching to ExternalFile leaves the text byte-identical.
- **Criterion 6: mode-scoped saving.** Two tests, one per mode, plus
  overlay absence in both.
- **Criterion 7: multi-site bindings.** Independent keys on the
  recording transport; persist and read back in both modes; a line
  move; a reorder migration; STALE on a broken mapping.
- **Criterion 8: reactive displays.** The store replays every 14.5.5
  batch shape (changing edge, failed diamond, provisional rollback,
  conditional unblocking, status recovery, late failure, ordinary
  failure then recovery), asserting render counts and that no
  provisional value is displayed.
- **Criterion 9: sampler waveform.** Handles, overlays and markers; the
  click-into-selection write; the no-selection no-op; the structural
  test.
- **Criterion 10: edit reconciliation.** Insert, delete and reorder
  above and inside a playing form; duplicates; `doc-changed`
  invalidation and a delayed write answered by `stale-binding`;
  declined write-back; the stored-list revision.
- **Criterion 11: visual pane.** The recording GL gets the program only
  after the boundary tick (real-wasm `TAG_RENDER` order) and the
  `TextAsset` texture; a GLSL failure keeps the previous program.
  Viewing the pane in a real browser is manual, pending user
  confirmation.
- **Criterion 12: Tauri.** `cargo check`, plus `npm run build`, `check`
  and `test`. The app run is manual.

**Waves.**
- The plans are `impl-plans/active/vactr-editor-<wave>.md`. The
  manifest is `impl-plans/active/ed-editor-20260926-s186-dispatch.json`.
  Evidence goes to
  `target/fe-logs/ed-<wave>-<check>-s<session>-<n>.log`, with the exit
  status recorded in each log.
- Each plan lists its own plan file in `writePaths`.
- Only FINAL edits `vactr-core.md`, `impl-plans/README.md` and the
  root `README.md`, and FINAL never edits the manifest.

| Wave | Content | Depends on |
|------|---------|------------|
| WIRE | Rust G2-G6: `session/{protocol,session,publish,editors,mod}.rs` (split `session.rs` at 800), `directives/{attach,mod}.rs` (G3 call arguments), `sched/{runtime,render,mod}.rs`, `pkg/{mem_cache,mod}.rs`, their tests; build-lsp and clippy-lsp included | — |
| SCAFFOLD | `package.json`, lockfile, tsconfig, vite and vitest configs, `index.html`, `editor/.gitignore`, `app/` shell with the stub `mount.ts` files, `protocol/` (all v1 + G-addition types, client, three transports), `platform/files.ts`, `worklet/host.js` options + `host.d.ts`, the mock clock and recording transport | — |
| WASM | Rust G1: `host/wasm/{session_half,mod}.rs` (+ a sibling file if 800 is reached), and its own real-wasm ABI smoke test `editor/test/wasm/abi.test.ts` (init, apply/eval, tick, check, frame, the pkg need/supply loop) | WIRE, SCAFFOLD |
| CODE | `code/` + its `mount.ts` | SCAFFOLD |
| MIDI | `midi/` + its `mount.ts`, the MIDI mock | SCAFFOLD |
| BIND | `bind/` + its `mount.ts` | CODE, MIDI |
| VISUAL | `visual/` + its `mount.ts`, the GL and canvas mocks | SCAFFOLD |
| PACKAGES | `pkg/` + its `mount.ts`, the OPFS and fetch mocks, the in-test zip builder | SCAFFOLD |
| PARAMS | `params/` + its `mount.ts` | BIND, VISUAL |
| TAURI | `editor/src-tauri/*` | SCAFFOLD |
| FINAL | `editor/test/wasm/*.test.ts` (real-wasm tests over G1-G6), `app/main.ts` wiring fixes, core-plan TASK-010 checkboxes, progress log and pending manual gates, `impl-plans/README.md`, root `README.md` | all |

Plans that share no dependency run in parallel in the one workspace.
When an editor-wide check fails only because a sibling is mid-edit,
that is recorded and the join re-verifies (issue #5 contract).

**Verification.** The 6.5.7 evidence rule applies to every plan.
- The Rust checks: `build`, `clippy` (`--all-targets -- -D warnings`),
  `fmt` (`--check`), `nextest`, `test` (plain
  `CARGO_TERM_QUIET=true cargo test`), `wasm32`, then `wasm32-hostwasm`,
  and `linecount` (the largest `.rs` under 800).
- The editor steps come after those: `npm ci`, `npm run check`,
  `npm run test`, `npm run build`.
- TAURI and FINAL add `cargo check --manifest-path
  editor/src-tauri/Cargo.toml` and `cargo fmt --check` for that crate.
- WIRE adds `build-lsp`, `clippy-lsp` and `tree-wasm32`. No new Rust
  crate enters the root tree.
- Every plan runs `npm run test`, `cargo test` and `nextest` (issue #5).

Manual gates, pending user confirmation with automated proxies:
- hearing the worklet audio in a real browser;
- the visual pane in a real browser;
- `cargo tauri build` and running the app.

Rollback is a `git revert` of the single implementation commit. OPFS
data is namespaced under `vactr-pkg/` and can be deleted, and
nothing migrates.

### 15.2 Editor UI on Solid.js, audio start, icons, foldable pane (author, 2026-09-26)

Decided after the first hands-on session, in which a document evaluated
cleanly in the engine but the page stayed silent with no visible reason
(the transport kept showing 120 bpm, so the eval never reached the
session or audio was never unlocked).

- **Solid.js for the UI.** Every panel (transport bar, right pane with
  the slider panel and directive control panel, parameter editors, step
  grid and piano roll, meters and scopes, sample browser, package pane,
  status/diagnostic surfaces) becomes a Solid component. CodeMirror 6
  was the code surface, mounted from a Solid component; 15.3 now replaces
  visible code rendering with GPU canvas. State comes
  from signals derived from the protocol client: one signal (or store
  path) per site, slot and telemetry stream, updated once per `bindings`
  batch / `playing` / `tempo` / `levels` message, so a batch repaints
  only the affected DOM (the 13/14.4 "one repaint per batch" rule holds
  by construction). Protocol, transport, bind write-back and render-host
  logic stay framework-free TypeScript modules; only views move. Build:
  `vite-plugin-solid`; tests: vitest + `@solidjs/testing-library` (jsdom).
- **Audio start and eval are never silent.** A visible audio-state
  control (states: *off* before the first gesture, *starting*, *running*,
  *suspended*, *failed* with reason) sits at the left of the transport;
  clicking it resumes the `AudioContext`. Evaluating while audio is off
  starts it (the eval keystroke is a user gesture). Every eval shows its
  outcome: the flash on the evaluated range plus a transient status
  (ok, n diagnostics, or not delivered), and a toolbar Run button
  evaluates the whole document for users who do not know the shortcut.
  Boot failures (wasm fetch, worklet `addModule`, session ABI missing)
  render in the status surface, never only in the console.
- **Icons instead of words in the transport.** Tempo = metronome icon +
  number; position = cycle.beat readout with a small beat ring; clock
  source = glyph (internal / MIDI locked / MIDI lost); hush = mute glyph;
  panic = stop glyph. Every icon has an accessible label and a tooltip
  carrying the old text. Icons are inline SVG in the bundle (no icon
  font, no network).
- **Foldable right pane.** The right pane collapses to a narrow rail
  (toggle button and a keyboard shortcut); the code pane takes the width;
  the folded state persists per viewer in `localStorage` (try/catch); the
  slider panel keeps its bindings while folded (no teardown of
  learned MIDI mappings). Sections inside the pane (sliders, directives,
  parameter editors, samples, packages) fold individually too.
- **Unchanged**: the protocol, the wasm and native tiers, Tauri wrapping
  the same `dist`, and every TASK-010 behavior; the TASK-010 vitest
  suites are ported, not dropped.

### 15.3 GPU canvas code editor and synchronized composition (2026-09-30)

**Issue reference:** `codex-design-and-implement-review-loop-session-224`.
**Status:** author proposal for independent adversarial review; no implementation,
performance, accessibility or physical-device acceptance is claimed here.
Historical issue #5 describes the baseline, not a newly supplied issue number.
**Amended:** 2026-10-05 by 15.3.8 (cutover amendment); where 15.3.8 names a
rule explicitly as superseding, it replaces the baseline text below.

#### 15.3.1 Baseline and boundary

Retain architecture.md Host Tiers and Editor Requirements, sections 14.4,
15.1 and 15.2, design-visual.md's Hydra chains, and the completed
`impl-plans/completed/vactr-editor-wasm.md` raw session ABI. This amendment
supersedes only visible CodeMirror rendering, DOM decoration dependencies,
receipt-based synchronized timing, and the earlier exclusion of native
frontend visual composition. Native engine visual-language execution remains
a separate capability: unsupported programs must report a diagnostic.
Solid panels, document protocol, binding authority, audio DSP and two-Wasm
browser delivery remain. DOM panels and input/accessibility bridges are allowed;
all visible source text, line numbers, cursor, selection, syntax, diagnostics,
evaluation flashes, playing spans and inline binding badges use WebGL2 canvas.
No second editor engine, new language coupling, raw GLSL interface, browser taps,
per-slot audio mixing, AUv3 or new external-code-agent adapter is required.

Repository evidence: `code/mount.ts` constructs EditorView and exports it in
`app/apis.ts`; `bind/{mount,drag,write,routing}.ts` and `params/{mount,roll}.ts`
consume it. `code/history.ts` retains 256 revisions/four UTF-8 indexes;
`code/highlight.ts` retains 4096 events but anchors native times at receipt.
`app/clock.ts` returns processing time; `src/session/publish.rs` emits tempo only
on changes, without a sample timestamp. `visual/render-host.ts` owns four
ping-pong outputs and release routines, but no editor glyph atlas or video
source. `src-tauri/src/main.rs` is presently a desktop wrapper without IPC.
The `code/`, `app/`, `bind/`, `params/` and `visual/` paths above are relative
to `editor/src/`; `src-tauri/` is relative to `editor/`.

#### 15.3.2 One editing authority and coherent migration

Retain CodeMirror EditorState, Text, ChangeSet and compatible state commands as
headless document machinery; do not instantiate a hidden EditorView as the
editing authority. A concrete code-surface contract in `app/apis.ts` exposes
state reads, transaction dispatch, change/selection subscriptions, focus,
coordinate-to-position and position-to-rectangle mapping, and range annotations.
It serves the existing consumers rather than providing a generalized editor
framework. `code/mount.ts` owns this surface, input bridge and renderer.

Each document-changing dispatch records `DocumentSync.apply` exactly once,
synchronously before subscriber notifications or protocol writes. Retain
revision/epoch increments, 200 ms debounce, flush-before-write/eval ordering,
UTF-8 wire versus UTF-16 state conversion, 256-revision touched-span rejection,
and four-index cache. Undo/redo are transactions through this same pipeline.
Selection-only and annotation updates do not increment document revisions.

Migrate sync listeners, eval key bindings/200 ms flash, diagnostic range data,
playing ranges, numeric drag hit testing and capture, directive write-back,
binding overlay badges, parameter-head selection and roll text reads together.
No consumer depends on EditorView DOM, decorations, lint gutter or widgets after
cutover. Preserve Mod-Enter form selection, Mod-Shift-Enter full eval, Mod-.
hush, source-edit versus runtime-overlay modes, stale-generation checks,
expected-text checks and per-target latest-wins rate limits. Syntax tokenization
is presentation only; sites and semantic identity still come from the session.

#### 15.3.3 Text, input and accessibility

Start with unwrapped lines, tabs at four space stops, horizontal/vertical scroll
and line numbers. Retain line-ending content in the document; rendering treats
CRLF as one line break. Navigation and hit testing use grapheme boundaries,
UTF-16 offsets and measured advances; never split surrogate pairs or combining
clusters. Shape and rasterize complete visible runs with browser font rendering
in an offscreen 2D canvas, upload bounded run tiles to an atlas, then draw only
GPU quads. This is a reusable text atlas, not per-frame full-page rasterization.
Cache keys include text, font/fallback identity, scale and syntax style; font
load/DPR changes invalidate metrics. Run-level shaping preserves ligatures and
Japanese glyphs; caret boundary advances must be validated against the same
shaping. Mixed RTL text requires visual-order hit testing and selection evidence;
if unsupported it is reported as an input limitation, not claimed supported.

Use a focusable textarea with a bounded surrounding-text window as the DOM
bridge. Keep it transparent at the caret rectangle (including visual viewport
and scroll offsets), available to accessibility and virtual keyboards, without
visible duplicate text. Reconcile beforeinput/input, composition and platform
selection changes into the single transaction pipeline. During composition,
show preedit and underline on GPU, retain replacement range, suppress eval,
shortcut dispatch and bridge reset, and commit exactly once at composition end;
cancellation restores the original state. Defer source-write operations touching
the composing range until composition ends, then revalidate expected text/site.

Clipboard cut/copy/paste uses selected document text and normal transactions;
respect platform event permissions and report failed operations. Keyboard
navigation supports word/line/document movement, shift extension, select-all,
undo/redo and scrolling to caret. Pointer hit testing supports click, word/line
selection, drag/autoscroll and numeric drag without stealing normal selection.
Touch supports caret placement, long-press word selection, GPU selection handles
and scroll gestures; pointer capture cancels on blur/cancel. VoiceOver/screen
readers receive label, editable value window, selection and bounded status
announcements; window shifts preserve document offsets. Review navigation across
window boundaries and full-document selection, not just the ARIA attributes.
Resizing, orientation, soft keyboard and DPR changes keep selection and scroll.

#### 15.3.4 Audible time, transport and revisions

Every frame samples one audible transport position shared by code highlights,
beat display and visual effects. Audio scheduling never waits for a frame.
Event timestamps mean scheduled processing-time onset in seconds in a declared
clock epoch. Source identity remains file + doc_revision + UTF-8 span; slot and
an optional sequence within epoch distinguish repeated events. Add optional
end-time seconds to playing records, calculated using the scheduled event's
original tempo; do not reinterpret queued durations with a later tempo.

Add periodic transport snapshots at no more than 20 Hz, independent of tempo
changes, containing epoch, sample processing time, cycle/beat, BPM,
beats-per-cycle, running/paused state and output-latency seconds with provenance
(measured, estimate, unavailable). Browser sample time comes from the audio
host; native time and position come from the same engine clock. New fields are
additive in protocol v1; legacy peers are explicitly marked unsynchronized.
Validate finite nonnegative latency/duration, positive BPM, valid rational
denominators, monotonic sample time within epoch, source bounds and message size.
An epoch changes on restart/seek/clock discontinuity; clear old pending events.

Browser audible time uses output-timestamp correlation between performance time
and audio context time when available. That correlation already represents
output presentation: do not subtract latency again. Without correlation, use
processing time minus a host-reported total output delay, explicitly estimated;
never add base/output delays blindly or claim compensation when unavailable.
Native IPC uses a bounded clock probe/reply with echoed page send time, engine
receive/send monotonic times, epoch and audio-output correlation/total delay.
Use the lowest-RTT valid sample to estimate page/engine offset, retaining at most
8 probes; refresh once per second. Reject RTT over 100 ms; expose uncertainty
including half RTT and host delay uncertainty. Receipt anchoring may support
legacy display only, labeled unsynchronized and excluded from synchronization
acceptance. Native WebSocket may carry the same probe contract for development.

Compute position from the latest valid timestamped snapshot and audible elapsed
time; hold on pause or lost external clock, reanchor on tempo changes and epochs.
Never advance beat or highlights by counting frames or message arrival deltas.
Hide synchronized indicators when snapshot age exceeds 2 seconds or correlation
is invalid; resume from fresh absolute time. On late frames discard expired
events, show still-active ranges and latest beat; do not replay missed flashes.
Hush/stop clears relevant queues. Unmappable/touched/too-old revisions drop spans.
Cap highlight entries at 4096, future horizon at 2 seconds, incoming batches at
4096 events, each telemetry envelope at 1 MiB, and native/frontend telemetry
backlog at 64 messages (coalesce snapshots/levels, discard expired events first);
record overflow/drop counters. Control requests/replies are separate: never drop
eval results, errors or document acknowledgments under the telemetry policy;
reject additional requests with a visible busy error at 64 in-flight requests.
A gap/overflow makes affected transient evidence incomplete, without blocking audio.

#### 15.3.5 Composition and resources

Reuse GlRenderHost for Hydra synth effects and o0..o3 feedback. Keep its GL
textures within their owning context. A code compositor may upload the latest
visual canvas image to one reusable background texture at most once per frame;
this explicit copy avoids claiming cross-context texture sharing or zero-copy.
Layer background/video, subdued contrast scrim, selections/playing/eval,
syntax text, diagnostic underlines, cursor and GPU badges/selection handles.
DOM tooltips/panels are allowed outside the source surface. Oscilloscopes reuse
published analyzer ring cells; beat display uses 15.3.4. No microphone capture.

Video scope is one user-selected local file, decoded by a media element and
composited as background; no camera, streaming service or new language syntax.
Autoplay follows user gesture; failed decode is visible, audio track is muted,
video wall/media time is not the musical clock. Musical uniforms/effects use
audible time. Reuse one texture, upload only new video frames, pause when hidden,
and revoke object URLs/release decoder on replacement or disposal. Cap uploaded
video to 1280x720 at 30 Hz; higher-resolution decoded memory must be measured
separately and rejected if the session resource budget cannot be met.

Initial enforceable caps per editor instance: text atlas 16 MiB, geometry and
staging buffers 8 MiB, text layout cache 8 MiB, canvas/backdrop targets 4 million
pixels total, visual outputs at most 1024x1024 each with eight RGBA8 targets,
and at most 8 MiB of visual text-asset textures. Total application-owned GPU
allocations including video remain below 96 MiB; reserve replacement allocations
against that cap before allocating. Use effective DPR reduction/downscale when
needed and show quality status. Validate device texture limits before allocation.
Caches evict offscreen LRU entries; visible working-set overflow renders in
bounded batches, never silently omits text. Idle frames change uniforms and
small feedback geometry only; dirty text/layout rebuilds occur only on edit,
scroll, font, viewport or atlas eviction. Reuse buffers and upload dirty regions.

Keep 256 history entries as a count ceiling and impose a 32 MiB history/undo
retention ceiling (evict oldest complete undo groups and report reduced depth),
with four UTF-8 indexes totaling at most 8 MiB. The current document is never
evicted; documents above the measured 1 MiB profile report unverified performance.
Account cached copies explicitly rather than assuming rope sharing bounds bytes.
Context loss cancels GPU work without changing the document or audio; restore
from CPU state with empty feedback targets and fresh resources. On allocation
failure retain editing state and accessible save, display GPU-unavailable status;
do not switch to visible DOM source. Resize releases superseded targets. Dispose
cancels frame/probe/video callbacks, listeners, observers, timers and captures,
then deletes textures, programs, buffers/framebuffers and bridge nodes.

#### 15.3.6 Browser and iPad/native delivery

Browser retains the existing real Rust/Wasm session and AudioWorklet ABI, with
new telemetry tested end-to-end. Desktop wrapper may keep the interim Wasm mode.
For native/iPad, convert the shell bootstrap to a shared Rust library entry
plus thin desktop main, add a Session Protocol IPC transport and native session
owner on a control thread using existing host capabilities. Connect native audio
through the existing Rust host; no VM, IPC, allocation or frontend work runs in
its callback. Advertise capabilities and fail clearly if audio init is unavailable.
Expected ownership is `editor/src/protocol/tauri.ts` for the IPC adapter,
`editor/src-tauri/src/{lib,session,clock}.rs` for shared bootstrap, session and
clock probe handling, and the existing `src/host/` audio and capability modules
for native host wiring. The plan resolves exact host paths after reading them,
without creating a separate engine. A session owner closes IPC subscriptions
and stops/joins its control task on app close. Foreground activate/suspend/close
controls session lifetime; iOS glue is limited
to audio-session activation, interruption/route changes and latency refresh.
Audio route changes invalidate clock correlation. No new Swift UI is introduced.

Deliver shared frontend transport, Rust session lifecycle/clock contract,
mobile library entry/configuration and integration tests that build on available
host tooling. The plan must specify exact files and native host wiring before
approval; a desktop compile alone is not an iPad build. Add iOS target compile,
packaging/signing and physical-device checklist; lack of SDK, signing or device
is a reported incomplete platform gate, never replaced by a mock pass. Device
checks include Japanese keyboard, touch handles, VoiceOver, audio unlock,
interruption/resume, orientation, high DPI and output-route latency. PWA on iPad
remains the browser delivery, not evidence of native audio integration.

#### 15.3.7 Verification and review gates

Budgets are proposed acceptance targets, not measured results. Before measurement
record actual hardware model/CPU/GPU/RAM, OS, browser/WebView versions, display
refresh/DPR, audio device/sample rate/buffer, build mode and workload seed. Measure
desktop browser and physical iPad separately; unknown hardware remains pending
in `user-qa/pending-editor-questions.md` E6. Use a 1 MiB/20,000-line document
including Japanese, emoji and long lines, 64 simultaneous sounding voices,
four 1024x1024 synth outputs, one 720p video and analyzer scope; also test text
alone to identify regressions. Warm up 30 seconds, record 5 minutes of editing,
selection and scrolling, then 10 minutes of repeated edit/font/DPR/resize cycles.

| Metric | Acceptance target and evidence |
|--------|--------------------------------|
| Frame | At 60 Hz, JS update/submission p95 <=8 ms and presentation interval p95 <=16.7 ms, p99 <=33.4 ms; record GPU time when supported separately |
| Input | Real input event to presented updated canvas p95 <=50 ms; report sampling method, composition separately |
| Sync | Audible reference impulse versus visible onset p95 <=33.4 ms when host correlation uncertainty <=10 ms; record external audio/video measurement and estimated-host cases separately |
| Late frame | Inject 250 ms stall; first recovered frame shows current beat/active ranges, no expired flash replay; no growing offset over 5 minutes |
| Resources | Caps above never exceeded; application-owned live allocations return to baseline on dispose; retained heap growth <=8 MiB after warmup/GC across 10-minute cycles, decoded video memory reported separately |
| Audio | UI stalls/hidden/context loss do not stop audio; record underruns under combined workload rather than infer independence from UI tests |

Unit tests cover revision conversions, stale spans, composition transaction
ordering, clipboard/undo semantics, grapheme coordinates, epoch/correlation and
latency double-counting, overflow and late recovery. Existing jsdom tests verify
contracts only. Real browser/WebGL evidence must prove visible glyphs and all
feedback are canvas-rendered, texture reuse, high DPI, context loss and input.
Japanese IME and accessibility require manual browser/device evidence with steps
and captured results. Preserve existing bind/params behavior tests while migrating.
Run `cd editor && npm run check`, `cd editor && npm run test`,
`cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`, real-Wasm tests,
`CARGO_TERM_QUIET=true cargo check`, clippy all-targets with warnings denied,
fmt check and nextest with the required failure-focused environment, plus
`CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml`.
The plan must list actual browser/mobile commands from installed tooling; never
invent a passing device or GPU command. Record complete logs and final statuses.

Independent design and code-free file-level plan review precede implementation.
The plan covers the consumer migration, telemetry producer/consumer pairing,
resource accounting, native wiring, dependencies and behavioral gates. Accepted
plan commit and non-force push precede implementation fanout; task-only final
commit/push follow review and improve self-review. Preserve recorded MusicDSP,
rumble and dirty shared-file hunks, assigning one owner for shared protocol edits.
Missing device/measurement evidence stays unchecked even if build checks pass.

#### 15.3.8 2026-10 cutover amendment

**Issue reference:** `workflow-input:opus-luna-design-and-implement-review-loop-session-265`
(no GitHub issue number). **Status:** author amendment for review (2026-10-05).
This is a fresh run. It supersedes the canvas-editor-224 plan chain, which is
historical only: its dispatch, snapshot, fingerprint and recovery contracts are
not resumed or repaired. Sections 15.3.1 to 15.3.7 remain the baseline. This
amendment fills six gaps found by audit: (a) song mode, (b) the completion
popup, (c) format and tree-sitter syntax, (d) the frame scheduler and
backgrounding, (e) ABI batch bounds and incremental deltas, and (f) the
measurement protocol. It also freezes the cross-plan contracts that the
canvas-cutover plans implement. Nothing here is a measured result.
Session 267 (resume of 266) adds to 15.3.8.8 the silent automated audio rule
(operator decision S), the head-only control run, the lowered workload
amplitude and two harness clarifications, and to 15.3.8.9 the owner-file
defect-fix rule. Nothing else changes.
Session 269 (resume of 267) adds 15.3.8.10: the bounded edit-path cost rule
(no whole-document string per keystroke or frame), the timing-budget rule for
wall-clock vitest gates, and selective redispatch as the alternative to the
owner-file fix. It changes no accepted contract.
Session 271 (resume of 269) adds 15.3.8.11: absolute wall-clock budgets move
out of the default parallel vitest run into a serial perf gate (operator
decision P). It replaces the "Wall-clock vitest gates" paragraph of 15.3.8.10
and changes no product contract.

##### 15.3.8.1 Baseline check (repository state at c9e5a05)

These facts were confirmed on 2026-10-05:

- Fifteen production files import `@codemirror/view`. `code/mount.ts` still
  builds an `EditorView` with `lineNumbers`, `lintGutter` and decorations.
  `CodeApi.surface` is optional, and production never sets it.
- The headless pieces exist and are tested in jsdom: `code/surface.ts`,
  `history.ts`, `input.ts`, `keyboard.ts`, `pointer.ts`, `accessibility.ts`,
  `renderer.ts`, `atlas.ts`, `layout.ts` and `resources.ts`. The renderer has
  no frame loop.
- `code/language.ts` (StreamLanguage, HighlightStyle) imports
  `@codemirror/language`, which imports `@codemirror/view` itself.
  `code/diagnostics.ts` imports `@codemirror/lint`.
- `app/clock.ts` returns `currentTime` or `performance.now()`.
  `code/highlight.ts` anchors native event times at receipt.
- `src/session/publish.rs` emits `TransportSample` with
  `latency_kind: "unavailable"`. `LevelsBody` has no timestamp.
- The native audio callback in `src/host/native/audio.rs` (981 lines) ignores
  `cpal::OutputCallbackInfo`. No `src/host/native/clock.rs` exists.
- `vactr serve` (`src/cli/serve.rs`) owns the only native session loop.
  `editor/src-tauri/src/main.rs` has 16 lines, no `lib.rs`, no IPC and no iOS
  project.
- The session-half Wasm outbox is drained after every ABI call. Its capacity is
  unfixed (`fix_outbox(0)`), so per-call output is the effective bound.
- `doc-changed` already carries composed byte changes rather than whole text.
- Only the `playwright` 1.62.1 library is installed. `@playwright/test` is not.

**Superseded baseline rules.** Each is replaced as stated below:

- 15.3.6 expected `editor/src-tauri/src/clock.rs` and a Tauri-specific probe
  command. Section 15.3.8.5 replaces these with a Session Protocol
  `clock-probe` that both native transports use.
- The 15.3.7 workload durations apply only to the physical-device and desktop
  product profile. Section 15.3.8.8 defines the shorter headless gating
  profile.
- The suggested separate CONSUMERS plan is merged into MOUNT
  (see 15.3.8.9).

##### 15.3.8.2 Frozen code-surface contract and consumer migration

`app/apis.ts` gets the final contract in the MOUNT plan. Offsets are UTF-16
and coordinates are viewport CSS pixels.

- `CodeApi.view` is removed, and `CodeApi.surface: CodeSurface` becomes
  required.
- `CodeSurfaceUpdate` adds `userEvent: string | null`, taken from the last
  transaction's `Transaction.userEvent`. Annotation-only updates carry `null`.
- `CodeSurface` adds three members:
  - `addKeymap(bindings: readonly { key: string; run(): boolean }[], precedence?: 'highest' | 'default'): () => void`
  - `onBlur(cb: () => void): () => void`
  - `onCompositionStart(cb: () => void): () => void`
- `CodeSurface` also adds
  `registerNumericDrag(provider: (event: PointerEvent, pos: number) => NumericGesture | null): () => void`.
  The `PointerController` consults these providers in its existing
  `numericDrag` option.
- `undo()` and `redo()` remain class methods on the surface, and so does
  `annotationRanges()`.
- The `CodeAnnotation.kind` union adds `'call-head'`. For this kind, `label`
  carries the params group id, and the renderer never draws it as text. It
  draws a subtle underline mark only.

The key notation is the CodeMirror subset already in use: `Mod-`, `Shift-`,
`Alt-`, `Ctrl-` and `Meta-` prefixes plus `KeyboardEvent.key`. If Alt or Shift
changes the produced character, matching falls back to `KeyboardEvent.code`
(so Shift-Alt-f still matches on macOS). `Mod` means Meta on Apple platforms
and Ctrl elsewhere.

Key dispatch runs in this order:

1. `highest` bindings, used by the completion popup.
2. `default` bindings, used by eval and format.
3. The built-in `KeyboardController` commands.

During composition, no keymap runs. A binding that returns true calls
`preventDefault`.

Selection-only and annotation updates still do not increment revisions.
`DocumentSync.extension()` (the EditorView listener) is deleted, so
`DocumentSync.apply` is reached only through `CodeSurface.dispatch`.

**Consumer migration rules.** These replace every EditorView use. Each rule
preserves the asserted behavior of the existing tests, and those tests are
ported to the surface rather than deleted.

- **(a) Song mode (`app/song.ts`).** `deps.code.surface.subscribe` is the only
  document observer, and the `EditorView.updateListener` branch is deleted.
  `applyWholeCode` reads `surface.state.doc`.
  `store.songDocumentChanged(file, revision)` runs on every `docChanged`
  update. A pending candidate whose revision differs flushes `doc-changed`
  immediately, as it does today. Song-transport `playing` events use the same
  highlight path as live events. `test/app/song.test.ts` drives
  `surface.dispatch`.
- **(b) Completion (`code/completion-view.ts`).** `attachCompletion(surface, engine)`
  builds the existing UI-agnostic `CompletionSurface` from the contract above:
  - `text` and `selection` come from `surface.state`.
  - `replace` is a single transaction with the existing completion `userEvent`
    and selection.
  - `caretRect(pos)` is `surface.coordsAtPos(pos)`, which comes from renderer
    layout and includes scroll and visual-viewport offsets.
  - `isComposing` is `surface.compositionRange !== null`.
  - The completion keys use `addKeymap(..., 'highest')`. Their handlers return
    false while the popup is closed.
  - `onBlur` and `onCompositionStart` come from the input bridge.
  - The popup host stays `document.body`. It is a DOM panel outside the source
    surface.
  - On a viewport change (scroll, resize, DPR) while the popup is open, the
    popup re-anchors on the next frame, and it closes if the caret is
    offscreen.

  `completion.ts`, `completion-popup.ts` and `completion-types.ts` keep their
  behavior. No `@codemirror/autocomplete` is added.
- **(c) Format (`code/format.ts`).**
  - `formatDocument(surface, formatter)` reads `surface.state`. It aborts if the
    text changed during the await, and otherwise dispatches one `minimalChange`
    transaction with `userEvent: 'format'` (a single undo step).
  - `formatKeymap` becomes a default-precedence `addKeymap` binding for
    `Shift-Alt-f`.
  - During composition, format is refused. It is not deferred.
- **(c) Syntax (`code/syntax.ts`, `syntax-core.ts`, `language.ts`).** Syntax
  becomes a span provider and is no longer a ViewPlugin.
  - **Tree-sitter.** The provider keeps one `ParsedVact`. After a document
    revision it reparses at most once per text phase (15.3.8.3) and is never
    run per transaction. It uses `tree.edit` plus `parser.parse(input, oldTree)`
    (incremental), with edits taken from the transaction change sets coalesced
    since the last parse. `input` is the chunked document read of 15.3.8.10,
    never a whole-document string. Any reparse error falls back to a full parse.
    `styleSpans` runs only for the visible line range plus one viewport of
    overscan.
  - **Fallback tokenizer.** Before tree-sitter loads, or after it fails to
    load, the fallback runs the existing `vactParser` token rules through a
    local line-stream class in `language.ts`, so `@codemirror/language` is not
    imported. Per-line start state (the `inString` flag) is cached up to the
    viewport end and truncated at the first changed line.
  - **Output.** Spans go to the renderer as `syntax` annotations, with a cap of
    16,384 spans per text phase. Text beyond the cap is still drawn, in the
    default color, and a `syntax-truncated` counter is recorded. Text is never
    omitted.
  - **Removed.** `vactLanguage()`, `vactHighlightStyle` and
    `treeSitterHighlighting(Extension)` are deleted. `codePane.dataset.syntax`
    (`fallback`/`tree-sitter`) is kept.
- **Eval (`code/eval.ts`).**
  - Mod-Enter (form), Mod-Shift-Enter (whole) and Mod-. (hush) become
    default-precedence keymap bindings.
  - The 200 ms flash becomes an animation-layer range (15.3.8.3) and is no
    longer a StateField decoration.
  - Flush-before-eval and the stale-generation checks stay unchanged.
- **Diagnostics (`code/diagnostics.ts`).**
  - `setDiagnostics`/`lintGutter` become
    `surface.annotate('diagnostics', ...)` with `kind: 'diagnostic'`, a
    severity `className` and the message `label`.
  - The GPU draws underlines and a severity marker in the line-number gutter.
  - Hovering an underlined range (through `onPointer` and `posAtCoords`) shows
    a DOM tooltip outside the text layer.
  - The transport-bar listing and the debounced local check stay unchanged.
  - Diagnostics count changes are announced through the accessibility status
    region (bounded).
- **Bind and params.** These rules cover every `CodeApi.view` use found by
  `grep -rn 'code\.view\|code?\.view' editor/src` at c9e5a05:
  `app/song.ts:78,127` (rule (a)), `bind/mount.ts:79-337`,
  `params/mount.ts:186,259,305,314` and `params/roll.ts:93`.
  - `bind/mount.ts` holds `code.surface` in place of `code.view`. The
    `text`/`slice` callbacks use `surface.state.sliceDoc`, `lineOf` uses
    `surface.state.doc.lineAt`, the whole-text and `Utf8Index` reads use
    `surface.state.doc.toString()`, and the writes use `surface.dispatch`. It
    passes the surface, not a view, to the drag and write dependencies.
  - Runtime-overlay values (the `bind/drag.ts` `OverlayWidget` point widget
    driven by `bind/mount.ts` `updateOverlays`) become zero-width
    `surface.annotate('bind-overlays', [{ from: pos, to: pos, kind: 'binding', label: ' = <value>' }])`.
    The GPU draws the label text right after `pos`. `setOverlays` and
    `overlayField` are deleted, and an empty list clears the overlays on
    dispose.
  - `bind/drag.ts` uses `registerNumericDrag`. Normal selection is untouched
    unless a provider returns a gesture.
  - `bind/write.ts` and `bind/routing.ts` dispatch through the surface and use
    `deferSourceWrite` for composing ranges.
  - **Parameter call heads (`params/mount.ts`).** The `headField` decoration,
    `setHeads` and the `EditorView.domEventHandlers` click handler are
    replaced:
    - `markHeads()` publishes `surface.annotate('params-heads', ...)`. Each
      entry has `kind: 'call-head'`, `className: 'params-call-head'` and
      `label` set to the group id, with the ranges sorted, non-empty and
      mapped through `mapWireSpan` at `evalRev` as today. Heads are re-marked
      only from an eval-result, as today, and `dispose()` clears the owner
      with an empty list.
    - Click-to-open subscribes to `surface.onPointer`. On a primary-button
      `pointerup` that moved at most 5 px since its `pointerdown`, with no
      numeric gesture active and no composition, the handler takes
      `posAtCoords` and looks for a `call-head` range containing it. On a
      hit, it runs `showTab('editors')` and `open(groupId)`.
    - The handler never calls `preventDefault`, so caret placement proceeds
      exactly as the current handler, which returns false, allows.
    - Numeric-drag providers have precedence. A gesture that started a
      numeric drag never opens a call head, and call heads (call names) do
      not overlap numeric sites.
    - `formText` reads `surface.state.sliceDoc(r.from, r.to)`.
  - **Pointer forwarding.** `code/mount.ts` forwards the canvas
    `pointerdown`, `pointermove`, `pointerup` and `pointercancel` events to
    `onPointer` subscribers before the `PointerController` handles them.
  - **Roll text (`params/roll.ts`).** `sourceText` reads
    `code.surface.state.sliceDoc(r.from, r.to)`.
  - **Test port (`test/params/open.test.ts`).** The port keeps both
    assertions:
    - The head-list assertion
      (`['peq', 'env-adsr', 'euclid', 'odd', 'wobble']`) reads the
      `call-head` entries of `surface.annotationRanges()` through `sliceDoc`.
    - The "click on a call head opens envelope-shape" assertion dispatches a
      synthetic `pointerdown`/`pointerup` pair inside the `env-adsr` head
      range. It uses a `posAtCoords` stub, because jsdom has no layout. The
      handle, open-button, `transport.sent` and text assertions stay
      unchanged.
  - All existing rate-limit, expected-text and stale-generation behavior stays
    unchanged.
- **Import guard.** A new vitest test, `editor/test/canvas/no-editor-view.test.ts`,
  scans every `editor/src/**/*.{ts,tsx}` file. It fails on any import of
  `@codemirror/view`, `@codemirror/lint`, `@codemirror/language` or
  `@codemirror/autocomplete`, and on any `EditorView` identifier.
  - `@codemirror/state` and `@codemirror/commands` (history, undo and
    `isolateHistory`) stay allowed.
  - `@codemirror/commands` declares `@codemirror/view` as a package
    dependency. Bundled module code is therefore not claimed to be
    view-free. The claim is limited to "no production import and no view
    instance".
  - `editor/package.json` and `package-lock.json` are not changed by this
    migration. Unused direct declarations are left in place to avoid
    lockfile churn.

##### 15.3.8.3 Frame scheduler, backgrounding, DPR and resize (d)

A new file, `code/frame.ts`, defines `FrameScheduler`. It is the single
`requestAnimationFrame` owner for the code pane, and `code/mount.ts` composes it
with the surface, `InputController`, `PointerController`, `CanvasRenderer` and
`ResourceLedger`.

The scheduler tracks two dirty classes:

- **Text-dirty** is triggered by: a document revision; a selection, cursor or
  composition change; a scroll or viewport change; size; DPR; font
  generation; atlas generation or eviction; syntax-span revision; and
  diagnostic or binding annotation changes.
- **Animation-active** is true when playing highlights are non-empty or
  changed, an eval flash is live, the caret blinks (530 ms phase), the beat
  indicator is visible, or the background visual canvas published a new frame.

The scheduler requests a frame only when something is dirty or active. An idle
editor requests no frames.

Each frame runs these steps in order:

1. Sample the audible clock once (15.3.8.4).
2. Update the highlight scheduler and the transport bar.
3. If text is dirty, run the text phase: re-layout the visible runs, upload
   missing atlas tiles, run the syntax provider, and rebuild and upload only
   the dirty geometry ranges into the reused buffer.
4. Composite: background texture (if it changed), selection, playing and eval
   quads, the cached text geometry, underlines, the cursor, badges and
   handles.

Animation-only frames reuse the cached text geometry and perform no shaping or
geometry upload. `renderer.stats.textBuilds` and `frames` prove the split.
Playing, eval and beat ranges go to the renderer's animation layer through the
scheduler. They do not use `surface.annotate`, so they cause no
surface-subscriber notification and no accessibility refresh at frame rate.

**Backgrounding.**

- On `visibilitychange` to hidden, or on the page-lifecycle `freeze` event, the
  scheduler cancels the pending rAF and schedules nothing more.
- The video pauses, and the visual loop uses its existing `setVisible(false)`.
- Audio is never suspended by the editor. Telemetry keeps arriving; its queues
  stay bounded, and expired entries are pruned against the audible clock on
  arrival.
- On becoming visible again (or on `resume`), the scheduler marks size and DPR
  for revalidation and requests one frame. That frame derives the active
  ranges, beat and visual time from the current absolute audible time.
  Expired events and flashes are dropped and never replayed.
- If the browser suspended the AudioContext, the clock reports that the
  correlation is invalid, and the indicators stay hidden until it is valid
  again.

**DPR and resize.**

- A `matchMedia('(resolution: <dpr>dppx)')` listener is re-registered after
  each change. It calls `renderer.resize(dpr)`, which recomputes the effective
  DPR under the 4-million-pixel target cap (and shows a quality status when the
  DPR is reduced), and invalidates the atlas.
- A `ResizeObserver` on the code pane, and `visualViewport` resize and scroll
  events, are coalesced into at most one backing-store resize per frame.
  Superseded targets are released.
- The virtual-keyboard bottom inset is
  `innerHeight - (visualViewport.height + visualViewport.offsetTop)`. It is a
  viewport change that keeps the caret visible above the inset.
- Selection and scroll offsets persist across these changes.

**Context loss and disposal.**

- WebGL context loss stops the text and composite phases. Editing, the
  accessibility bridge and save keep working. Restore rebuilds GPU resources
  from CPU state and marks everything dirty.
- If GPU init fails, the renderer reports `unavailable`. The status shows that
  editing and save remain available. No visible DOM source fallback is shown.
- `dispose()` cancels the rAF and timers, disconnects observers and media
  listeners, releases pointer captures and the bridge nodes, deletes GPU
  objects, and returns the ledger's `usedBytes` to 0.

**Perf instrumentation.** The scheduler keeps a fixed ring of 4,096 frame
records: rAF timestamp, work duration, text phase yes/no, and the document
revision presented. It also records key-event timestamps. It is enabled only
by `?perf=1` and exposed as `window.__vactrPerf`. When disabled, it performs no
per-frame allocation.

##### 15.3.8.4 Audible clock and telemetry contract

`app/clock.ts` adds `AudibleClock`. `app/deps.ts` adds the optional field
`EditorDeps.audible?: AudibleClock`. `deps.clock: Clock` keeps its current
meaning, processing time, so MIDI learn and forward and every other scheduling
user is unchanged.

`AudibleClock` provides two methods:

- `now()` returns the audible processing time, in seconds, at
  `performance.now()`.
- `sample(frameMs)` returns `{ time, epoch, provenance, uncertainty, valid }`
  for the expected presentation time, `frameMs + lead`. The lead is the EMA of
  rAF intervals, clamped to [8.3, 33.4] ms. Results are cached per `frameMs`,
  so every rAF callback in one frame (code pane, visual loop, transport bar)
  sees the identical value.

`boot` creates the clock for each tier. Display users (highlights, beat, Hydra
uniforms, scopes) read `deps.audible`. When `deps.audible` is absent (test
fixtures), they wrap `deps.clock` with provenance `unavailable`, which
preserves the current behavior.

**Correlation sources:**

- **Browser.** When `ctx.getOutputTimestamp()` returns finite positive
  `contextTime` and `performanceTime`, `|pageMs - performanceTime| <= 1000`,
  and `contextTime <= currentTime`, the audible time at `pageMs` is
  `contextTime + (pageMs - performanceTime) / 1000`, with provenance
  `measured` and uncertainty of one render quantum. No latency is subtracted
  again.
  - Otherwise, if `ctx.outputLatency` is finite and positive, the time is
    `currentTime - outputLatency`, with provenance `estimate`.
  - Otherwise the time is `currentTime`, with provenance `unavailable`
    (uncompensated, and excluded from sync acceptance).
  - The CLOCK plan must confirm that browser `playing` and `TransportSample`
    times use the AudioContext processing-time domain of the session tick.
    Any other domain is a blocking contradiction to report.
- **Native, over IPC or WebSocket.** `ProbeCorrelation` sends the Session
  Protocol request
  `clock-probe { page_time_ms }` once per second. The reply is
  `clock-probe-reply { page_time_ms, processing_time, epoch, latency_seconds, latency_kind, uncertainty_seconds }`
  and uses the existing request/reply correlation. It is a control reply, so
  it is never dropped as telemetry.
  - At most 8 samples are kept, and samples with RTT over 100 ms are rejected.
  - From the minimum-RTT sample:
    `offset = processing_time - (page_time_ms + rtt / 2) / 1000`.
  - The audible time is then `pageMs / 1000 + offset - latency_seconds`.
  - The uncertainty is `rtt / 2 + uncertainty_seconds`, and the provenance is
    the reply's `latency_kind`.
  - Samples reset on an epoch change. The correlation is invalid if no valid
    sample has arrived for 3 s.
  - Receipt anchoring (`TimeAnchor` `receipt`) remains only for legacy peers
    that lack probe support. It is labeled unsynchronized.

**Transport position and indicators.**

- **Position.** The position comes from the latest `TransportSample` in the
  current epoch with `sample_time <= audible`. If none qualifies, the earliest
  sample is extrapolated backward, by at most 1 s. While running,
  `cycle = s.cycle + (audible - s.sample_time) * bpm / 60 / beats_per_cycle`.
  While paused, the cycle holds. It is never advanced by counting frames.
- **Hiding.** Indicators hide when the snapshot is older than 2 s or the
  correlation is invalid.
- **Beat flash.** The beat flash is visible only while
  `audible - beatTime` is in [0, 0.08) s. A beat whose window passed between
  frames never flashes.
- **Playing highlights.**
  - An entry is active while `time <= audible < end_time`. Legacy events
    without `end_time` use `durSeconds` with the tempo at receipt.
  - Entries whose epoch does not match are dropped, and so are entries more
    than 2 s in the future.
  - There are at most 4,096 entries, and hush clears them.
- **Eval flash.** The eval flash is UI feedback timed on page time (200 ms),
  not on the audio clock. It is not replayed after a stall.

**Additive protocol fields** (protocol v1, all optional):

- The `clock-probe` and `clock-probe-reply` pair.
- `LevelsBody.time`: the processing time of the publishing tick, which is
  `host_now`.
- `LevelsBody.epoch`.

Ownership is split by language. The Rust producer side (`protocol.rs`,
`codec.rs`, `publish.rs`, `session.rs`) belongs to NATIVE. The TypeScript side
(`protocol/types.ts`, `envelope.ts` validation, `client.ts` `clockProbe`)
belongs to CLOCK. Each side tests against the literal JSON above. Validation
requires finite, non-negative times and latencies, and an envelope of at most
1 MiB.

**Deterministic tests** (`editor/test/canvas/clock.test.ts` and ported
highlight and transport tests) use a simulated page and context clock, 60 Hz
frames with random drops, and injected 250 ms stalls. They assert:

- Each frame's active ranges equal the analytic set.
- Beat phase error stays at or below 1e-9 cycles over 5 simulated minutes.
- The number of flashes equals the number of beat windows that intersect a
  frame target, so missed beats produce no flash.
- An epoch change clears pending entries.
- Latency is not double-subtracted when `getOutputTimestamp` is valid.
- Probe sampling selects the minimum RTT, rejects RTT over 100 ms, and hides
  indicators once data is stale.

##### 15.3.8.5 Native clock, Tauri desktop IPC and iOS shell

**`src/host/native/clock.rs` (new).** `OutputClock` is a seqlock of
`AtomicU64` values: sequence number, frames at callback start, callback
`Instant` in nanoseconds since the clock origin, and the playback-minus-callback
duration in nanoseconds.

The cpal output callback writes it:

- The callback calls `record(&info, frames_before)`, which does one
  `Instant::now()` and five atomic stores. It never allocates, locks, or waits
  on rendering.
- The playback duration is `info.timestamp().playback.duration_since(&callback)`.

The owner thread reads it:

- `read()` returns an `OutputClockReading`:
  `{ processing_time, latency_seconds: Option<f64>, latency_kind, uncertainty_seconds: Option<f64> }`.
  The processing time is extrapolated as the frames at the last callback
  divided by the sample rate, plus `min(elapsed, one buffer period)`.
- A torn read retries up to 4 times, then reuses the previous reading.
- The latency kind is assigned as follows:
  - `measured`: the host-reported playback timestamp is present, and the
    reading is at most 1 s old.
  - `estimate`: there is no usable playback timestamp, so the value is the
    buffer frames divided by the sample rate.
  - `unavailable`: there is no stream, the stream is interrupted, or the
    reading is stale.
- The uncertainty is one buffer period.

`measured` means host-reported. cpal on CoreAudio may exclude some device or
safety-offset latency, so a physical acoustic check is listed as pending.

**Native host changes.**

- `audio.rs` is touched to add the callback hook. Because it would reach 1,000
  lines or more, the cpal stream construction (`open_with_outputs`) moves to a
  new submodule, `src/host/native/audio/stream.rs`.
- `NativeHosts` exposes the `OutputClock` and an owner-thread-only stream
  control with `suspend()` and `resume()` (cpal pause and play).
- If resume fails, a visible diagnostic says that audio is unavailable and
  the session must be restarted. Stream rebuilding is out of scope.

**Session changes.**

- A new `Session::observe_clock(reading)` stores the latest reading.
  `publish.rs` fills the `TransportSample` latency fields from it. When it is
  absent (the browser Wasm tier), the existing `unavailable` output is kept.
- The session answers `clock-probe` from the reading that was observed
  immediately before the request was applied.
- `Session::clock_discontinuity()` starts a new transport epoch, clears
  pending telemetry, and is used for interruption, resume and route change.

**Session owner (`src/cli/owner.rs`, new public module).** `SessionOwner`
spawns one control thread that builds the session (through the existing
`build_session`) and runs the loop that is now inside `serve.rs`. `serve.rs`
is reduced to the WebSocket adapter on top of this loop, so the CLI and Tauri
share one loop.

The loop:

- applies texts and connection events from an `mpsc` channel;
- before each `apply_text` and each 5 ms tick, calls
  `observe_clock(output_clock.read())`;
- handles `AudioInterrupted`, `AudioResumed` and `RouteChanged` events;
- shuts down when it receives `Shutdown` and joins the thread.

The audio callback never runs any of this work.

**Desktop Tauri.** `editor/src-tauri/src/lib.rs` (new) holds the shared
bootstrap, and `main.rs` becomes a thin call to `vactr_editor_lib::run()`. A
new file, `src-tauri/src/session.rs`, provides three commands:

- `session_connect(channel: tauri::ipc::Channel<String>) -> u32`, where each
  routed envelope becomes one channel message;
- `session_send(id, text)`;
- `session_close(id)`.

The owner is created lazily on the first connect and shut down on
`RunEvent::Exit`. The capability allowlist adds only these commands. If the
owner cannot start (for example, audio is unavailable), `session_connect`
rejects with the diagnostic.

`editor/src/protocol/tauri.ts` (new) implements `Transport` over `invoke` and
`Channel` from the already-pinned `@tauri-apps/api`. `app/main.ts` selects the
native tier with `TauriTransport` and `ProbeCorrelation` when `isTauri(win)`.
If the connect is rejected, it shows the error and falls back to the browser
Wasm tier. The `?session=` WebSocket tier keeps working and uses the same
probe. `editor/src-tauri/Cargo.toml` gains
`[lib] crate-type = ["staticlib", "cdylib", "rlib"]` and no new dependencies.

**iOS.** The iOS project is generated with
`tmp/canvas/tools/cargo-tauri ios init` (tauri-cli 2.11.5) and committed under
`editor/src-tauri/gen/apple/`, excluding build outputs. The SHELL plan
enumerates the generated files by a scratch init under `tmp/` before it fixes
its writePaths.

- **Entry point.** `lib.rs` adds `#[cfg_attr(mobile, tauri::mobile_entry_point)]`.
- **Audio session glue.** This lives only in the generated `main.mm`, as
  Objective-C before `start_app`:
  - set the `AVAudioSession` category to playback and make it active;
  - observe interruption-began, interruption-ended (with `shouldResume`) and
    route-change notifications;
  - forward each to the Rust C ABI `vactr_audio_session_event(kind: u32)`,
    exported from `lib.rs` for iOS only, which posts the matching owner event.
- **Background audio.** `Info.plist` sets `UIBackgroundModes = [audio]`, so
  audio continues in the background while the WebView stops its rAF (handled
  by 15.3.8.3).
- **Safe area.** `editor/index.html` gets `viewport-fit=cover`, and
  `env(safe-area-inset-*)` padding is applied to the app root.
- **Self-check.** When `VACTR_SELF_CHECK=1` (passed as
  `SIMCTL_CHILD_VACTR_SELF_CHECK=1`), the frontend invokes a `self_check`
  command once after mount. It reports tier, WebGL2, renderer status,
  effective DPR, audio-session state and latency kind. Rust prints that as one
  JSON line to stderr, which `xcrun simctl launch --console-pty` captures.
- **Restrictions.** There is no Swift UI, no provisioning profile, no signing
  change and no upload. Only simulator builds are made (`cargo tauri ios build`
  for `aarch64-sim`, or `xcodebuild -sdk iphonesimulator CODE_SIGNING_ALLOWED=NO`),
  and the exact command is recorded.
- **Toolchain.** Rust 1.83.0 is tried first. If the simulator build fails only
  because of the toolchain, a per-command `RUSTUP_TOOLCHAIN=1.98.1` is used for
  that iOS build alone and recorded as a limitation. All repository gates
  stay on 1.83.0.
- **Crate check.** If `vactr` `host-native` (cpal, midir) does not compile for
  `aarch64-apple-ios-sim`, the SHELL plan gates only the failing native
  capability by `target_os = "ios"` and reports it as unavailable. It does not
  stub audio.

##### 15.3.8.6 Visual composition

The Hydra frame loop (`visual/frame.ts`) calls `core.frame(t)` and
`host.draw(t)` with `t = audibleClock.sample(frameMs).time`, so the musical
uniforms follow audible time. It keeps its own rAF and its existing visibility
pause.

**Scopes (`visual/scopes.ts`).** Scopes keep up to 8 timestamped level frames,
in latest-wins order. Each frame presents the newest one whose `time` is at or
before the audible time. Frames without a timestamp are shown immediately and
labeled unsynchronized. Frames older than 2 s hide.

**Video background (`visual/video.ts`, new).**

- **Source and playback.** Video comes from one user-selected local file
  chosen with a DOM file input in the visual pane. It plays through a muted,
  `playsInline`, looping `HTMLVideoElement` from an object URL. Autoplay
  starts after a user gesture, and decode failure is shown in the visual pane
  status. Video time is never the musical clock.
- **Upload.** New frames are detected with `requestVideoFrameCallback`, or by a
  `currentTime` change when that is unavailable. Each new frame is uploaded to
  one reused texture at no more than 30 Hz. Frames larger than 1280x720 are
  aspect-fit downscaled through one reused 2D canvas. The 1280x720x4 bytes are
  reserved in the ledger before allocation.
- **Pause and disposal.** Playback pauses when the page is hidden. Replacement
  or disposal revokes the URL, clears `src`, calls `load()` to release the
  decoder, and deletes the texture.
- **Composition.** `GlRenderHost` draws the video as the base layer of the
  visual canvas. Hydra output `o0` is drawn over it using its alpha, so
  programs with alpha below 1 reveal the video, and the default opaque output
  covers it. This adds no language syntax.

**Backdrop.** The existing `VisualApi.onBackgroundCanvas(cb)` stays unchanged.
The visual mount calls `cb(canvas)` again after each newly drawn frame, and the
editor renderer treats each call as a new background revision. It uploads at
most once per frame, and only after a call. MOUNT wires this subscription
lazily, because `deps.visual` mounts after `code`. The beat indicator remains
the transport-bar DOM panel driven by 15.3.8.4.

##### 15.3.8.7 ABI and transfer bounds (e)

These bounds are enforced and tested:

| Path | Bound |
|------|-------|
| Document deltas | `doc-changed` carries only changed byte ranges and inserted bytes, composed per 200 ms debounce or forced flush. A test asserts that a one-character edit in a 1 MiB document yields a `doc-changed` payload under 256 bytes, with no whole-text transfer. |
| Allowed whole-text transfers | Only these: open/reset, song apply, format, completion request, debounced local check. Completion and check allow at most one request in flight (latest wins), are skipped while composing, and never run per frame. Check is also skipped when the revision is unchanged. |
| JS to Wasm per animation frame | Exactly one `session_frame` from the visual loop. No document text. |
| Wasm to JS | The outbox is drained synchronously per ABI call. Per drained batch: playing events at most 4,096 and telemetry envelopes at most 1 MiB (both already validated in `envelope.ts`). The telemetry backlog stays at 64 (the existing `MAX_TELEMETRY_QUEUE`: coalesce tempo and levels, drop expired playing first). Control replies are never dropped. In-flight requests stay capped at 64 (the existing `MAX_PENDING_REQUESTS` busy rejection). Tests assert these existing caps under the 4,096-event load. |
| Native IPC | One channel message per routed envelope. Per 5 ms tick, at most one tempo, one levels, and playing events up to 4,096. Probe replies and other control replies are never coalesced. |
| Frontend queues | Highlight entries at most 4,096, future horizon 2 s, scope frames 8, probe samples 8. Overflow and drop counters are exposed in status and in `__vactrPerf`. |
| GPU per frame | Atlas tile uploads at most 1 MiB (the remainder draws on later frames under a `text-pending` status, and is never omitted); geometry uploads only dirty ranges within the 8 MiB staging cap; background canvas upload at most 1; video upload at most 1 per new video frame, at most 30 Hz. |
| GPU and CPU caps | As in 15.3.5, enforced by `ResourceLedger` reservation tests: atlas 16 MiB, geometry and staging 8 MiB, layout cache 8 MiB, targets 4 million pixels, total at most 96 MiB, history 32 MiB, UTF-8 indexes 8 MiB. |

##### 15.3.8.8 Measurement protocol and thresholds (f)

**Harness.** The harness is a set of plain Node scripts in `editor/test/e2e/`
that use the installed `playwright` library. It adds no `@playwright/test`
and no new dependency. The scripts serve the built `dist` from a `node:http`
server on 127.0.0.1 with an ephemeral port, and run outside the Codex sandbox.
An `e2e` entry in `package.json` `scripts` is allowed and is not a dependency
change.

**Behavioral checks** (Chromium and WebKit, each pass or fail):

- **Canvas-only text.** No visible DOM text node in the code pane contains
  document text. The bridge textarea is transparent. Canvas pixel readback
  shows glyph pixels at line positions, and hiding the canvas leaves no visible
  source.
- **Editing.** Typing, undo and redo, and word, line and document navigation
  work.
- **Clipboard.** On Chromium, clipboard permissions are granted and the real
  clipboard is used. On WebKit, synthetic `ClipboardEvent` events are used,
  and this is labeled as a limitation.
- **IME.** On Chromium, Japanese composition is driven with CDP
  `Input.imeSetComposition` and `Input.insertText`. On WebKit, only synthetic
  composition events are possible, which is a limitation.
- **Touch.** Contexts use `hasTouch`. Tap and long-press work on Chromium
  through CDP touch events. WebKit uses synthetic pointer events (a
  limitation).
- **Context loss.** `WEBGL_lose_context` loses the context and then restores
  it, and the document is unchanged afterward.
- **DPR.** A deviceScaleFactor of 1 and of 2, plus a CDP metrics override in
  the middle of a run on Chromium. The CDP session that issued
  `Emulation.setDeviceMetricsOverride` stays attached until the DPR assertions
  (and, in the cycle run, the last DPR step) complete, because detaching it
  clears the override.
- **Resize and keyboard.** Window resize, plus a `visualViewport` inset
  simulated by viewport resizing.
- **Backgrounding.** A synthetic `visibilitychange` must leave zero rAF
  callbacks while hidden and produce no replayed flash on return.
- **Disposal.** After dispose, `usedBytes` returns to 0.

**Workload.** A seeded generator, `editor/test/e2e/fixtures/large-doc.mjs`,
builds a 20,000-line document of 1.0 MiB plus or minus 5% of UTF-8. It
includes Japanese, emoji, at least 50 lines over 400 columns, and a leading
evaluable program with 64 sounding voices and four 1024x1024 synth outputs.
The 720p video is produced in the page by `MediaRecorder` from a canvas and
then loaded through the same object-URL path as a user file, so no binary
fixture is committed. The scopes stay active throughout.

The sounding voices use a built-in synth `inst` (no external samples). Its
amplitude is lowered so that 64 voices of 2 notes do not saturate: at most
`amp = 0.005` per voice, which bounds a coherent 128-saw sum at 0.64 (about
-3.9 dBFS) before any engine gain. The 64-voice, 2-note load is kept for load
realism. The measured pre-sink peak must be at most -1 dBFS and is recorded.

**Head-only control run.** Before the large-document run, the same page
evaluates only the workload head (the `inst` definition and the `stack ... > d1`
line), started by the same start method the large-document run uses. The
control proves the method and the voice sound: at least one onset and a
pre-sink peak above -60 dBFS. Control onsets are excluded from workload
counts. If the large-document run then shows zero onsets, the failure is
attributed to the large document (product cost or defect to triage), not to
the harness. If the control itself fails, the run is a harness failure.

**Silent automated audio (operator decision S, session 267).** No automated
run may produce audible sound. The production master output stage is not
changed, no system audio driver is installed, and the macOS system volume is
never read or changed.

- **Browser virtual sink.** Every Playwright context, for Chromium and WebKit,
  in `behavior.mjs`, `measure.mjs` and the control run, gets one init script
  that runs before any page script. It wraps `AudioNode.prototype.connect` so
  that a connection whose target is a realtime `AudioContext`'s
  `AudioDestinationNode` is redirected into a per-context sink:
  `app output -> pre-sink AnalyserNode -> GainNode(constructed with gain 0, never automated) -> post-sink
  AnalyserNode -> real destination`. The real destination stays in the graph,
  so the device clock, `outputLatency` and `getOutputTimestamp` remain the ones
  the product sees. The pre-sink meter polls `getFloatTimeDomainData` at an
  interval shorter than its window (fftSize 32768) and records peak dBFS, RMS
  and onset times and count (an onset is a 10 ms RMS block above -40 dBFS
  after at least 50 ms below -50 dBFS). The post-sink meter records peak.
- **Silence assertions.** Each run asserts: the sink was installed before the
  first destination connection; the count of direct (unwrapped) destination
  connections is 0; the post-sink peak is exactly 0. A violation suspends the
  AudioContext immediately and fails the run. The pre-sink peak dBFS, RMS and
  onset count and times are reported per browser in the evidence.
- **Chromium defense in depth.** Chromium launches with `--mute-audio`.
  WebKit has no equivalent flag, so the sink and its assertions are its guard.
- **Simulator and desktop shell.** Automated Tauri launches (the iPad
  simulator through `VACTR_SELF_CHECK=1`, and any desktop automation) never
  evaluate a document or start playback; an opened output stream renders
  silence. The self-check report adds the count of `playing` envelopes
  received before the report, and `ios-sim.mjs` asserts it is 0 in addition to
  the self-check line. No new launch flag is introduced unless this proves
  impossible; any such flag is test-only, off by default, documented, and
  listed in the plan's sharedPaths.
- **Native tests.** Automated Rust tests never open a real output device.
  `NativeHosts::open_with_bus_names` (reached from `src/cli/mod.rs` with the
  default `--host native`) is the only output-opening path; every test that
  spawns the CLI uses `--host noop`, and the Tauri session tests use
  `HostChoice::Noop`. The evidence plan records this audit and fixes any test
  that does not.

**Headless gating profile H** runs per browser on the recorded host
(Mac16,12, M4, 32 GiB): 10 s warmup, then a 60 s editing run (at least 500
keystrokes at about 10/s, with scrolling and selection; the floor counts all
editing-run keystrokes, while the count of keys paired with a presented
document revision is reported separately and must be above 0), and a 120 s cycle
run (edit, font, DPR and resize every 5 s, with a 250 ms main-thread stall
injected every 10 s).

| Metric | Definition | Pass threshold (profile H) |
|--------|------------|----------------------------|
| Input latency | From keydown `event.timeStamp` to the rAF timestamp of the frame after the frame whose text phase presented that revision (a presentation proxy). IME is reported separately. | p95 <= 50 ms, p99 <= 100 ms |
| Frame work | Duration of the code-pane frame callback, reported for animation-only and text-dirty frames | Animation-only: p50 <= 4 ms, p95 <= 8 ms, p99 <= 16.7 ms. Text-dirty: p95 <= 16.7 ms |
| Frame interval | rAF timestamp deltas, excluding injected stalls | p95 <= 20 ms, p99 <= 50 ms |
| A/V sync, model | For each onset: presentation-proxy time of the first frame showing the range, minus the onset mapped to page time through the active correlation | Gated only when provenance is `measured`: p95 of the absolute value <= 33.4 ms, p99 <= 50 ms, no highlight earlier than 2 ms before the onset. `estimate` is reported but not gated. `unavailable` is recorded as a limitation. |
| Late frames | The first frame after each stall | Active set equals the analytic set, 0 replayed flashes, absolute beat-phase drift at the end of the run <= 1 ms |
| Memory | `ResourceLedger` maximum per cap and total. JS heap on Chromium after CDP `HeapProfiler.collectGarbage` (WebKit has no heap API: ledger only, a limitation). | No cap exceeded. Ledger total <= 96 MiB. Heap growth from end of warmup to end of the cycle run <= 8 MiB. Ledger returns to 0 on dispose. |
| Audio | Worklet underrun and drop counters during stalls, hidden periods and context loss | Recorded. Any underrun attributable to a UI stall is a failure. |

**Platform fallbacks.**

- **WebGL2.** If headless WebKit lacks WebGL2, WebKit runs headed
  (`headless: false`) on the same host, and the run records the mode. If WebGL2
  is still unavailable, the GPU-unavailable path is verified and every WebKit
  GPU metric is recorded as a limitation.
- **Audio.** If an AudioContext cannot run (headless audio), its sync and audio
  metrics are recorded as unavailable.
- **Software GL.** If a browser renders with software GL, it is reported, and
  failing frame metrics are kept as recorded failures. They are never
  relabeled as passes.

**iPad simulator profile S.** Records: the build log, `simctl install` and
`launch` on a named iPad simulator, the self-check JSON line, and a
`simctl io screenshot`. Performance in the simulator is not representative,
so it is not gated.

**Product profile.** This uses the 15.3.7 table, durations and E6 equipment,
on a physical iPad and on desktop with external acoustic capture. It stays
pending. The evidence document lists each pending check with its procedure:

- Japanese hardware and software keyboard IME;
- touch handles;
- VoiceOver;
- audio unlock, interruption and resume;
- route-change latency (headphones and Bluetooth);
- orientation;
- 120 Hz frame time;
- sustained thermal animation;
- physical A/V sync by camera and microphone capture.

**Evidence document.** `design-docs/specs/design-canvas-editor-evidence.md`
records:

- host, browser, OS and Xcode versions, the run id and the exact commands;
- every metric, compared against its threshold, per platform;
- the raw per-sample JSONL under
  `design-docs/specs/evidence/canvas-cutover/<run-id>/`, at most 2 MiB
  committed, with full logs under `tmp/canvas/evidence/<run-id>/`;
- the platform limitations;
- the pending physical-hardware checks;
- that every automated run was silent, how silence was enforced and
  asserted, and the measured pre-sink levels.

##### 15.3.8.9 Plans, waves, gates and closeout

The new manifest is `impl-plans/active/canvas-cutover-dispatch.json`. Each plan
lists concrete file writePaths and sharedPaths only, with no directories. The
merge of CONSUMERS into MOUNT is intentional: removing `CodeApi.view` breaks
every consumer at once, so the suite can stay green only if the cutover and
the consumer ports land together.

| Wave | Plan | Depends on | Scope (owned files, summarized) |
|------|------|------------|---------------------------------|
| 1 | `canvas-cutover-clock` | none | `app/clock.ts`, `app/deps.ts`, `app/main.ts` (clock wiring), `code/highlight.ts` (timing only; the decoration exports stay for MOUNT), `code/transport.ts`, `code/mount.ts` (clock wiring only), `protocol/{types,envelope,client}.ts`, clock, highlight and transport tests |
| 1 | `canvas-cutover-native` | none | `src/host/native/{clock.rs,audio.rs,audio/stream.rs,mod.rs}`, `src/session/{publish,protocol,codec,session}.rs`, `src/cli/{owner.rs,serve.rs,mod.rs}`, `editor/src-tauri/src/{main,lib,session}.rs`, `editor/src-tauri/Cargo.toml`, `editor/src-tauri/capabilities/default.json`, `editor/src/protocol/tauri.ts`, Rust and TypeScript tests |
| 2 | `canvas-cutover-mount` | clock | `code/*` cutover (adds `frame.ts`; the files named in 15.3.8.2 and 15.3.8.3), `app/apis.ts`, `app/song.ts`, `bind/{mount,drag,write,routing}.ts`, `params/{mount,roll}.ts`, the ported tests (including `test/params/open.test.ts`) and `test/bind/fixtures.ts`, `test/canvas/no-editor-view.test.ts`. Must not edit `app/main.ts`. |
| 2 | `canvas-cutover-visual` | clock | `visual/{frame,mount,render-host,scopes,video}.ts` (plus a visual-pane file input in an existing visual file), visual tests |
| 2 | `canvas-cutover-shell` | native, clock | `app/main.ts` (Tauri tier selection), `editor/index.html`, `editor/src-tauri/src/lib.rs` (mobile entry, C ABI, `self_check`), `editor/src-tauri/tauri.conf.json`, the enumerated `editor/src-tauri/gen/apple/*` files, `test/app/main.test.ts` |
| 3 | `canvas-cutover-evidence` | all | `editor/test/e2e/*.mjs` and fixtures, `editor/package.json` (`scripts` only), the evidence document and raw data, then closeout |

**Green after every plan.** Each plan ends with focused checks inside the
sandbox. Verification outside the sandbox runs the 15.3.7 command set plus:

- `cargo build --lib --target wasm32-unknown-unknown`;
- full nextest with a timeout of at least 1500 s (a timeout kill counts as
  neither a pass nor a failure);
- `cargo check --manifest-path editor/src-tauri/Cargo.toml`;
- rustfmt `--check` on touched Rust files only;
- for shell: the simulator build and launch;
- for evidence: the e2e harness;
- from session 271: `cd editor && npm run test:perf`, run alone (15.3.8.11).

**Other rules.**

- Clippy `-D warnings` must pass, with no new `allow` or `expect`.
- A touched source file of 1,000 lines or more is split.
- The audio callback and worklet never wait on, allocate for, or get driven
  by rendering.
- A product defect found by the evidence harness (for example the Chromium
  1x1 canvas backing, or a Run start that times out on the large document) is
  fixed in its owning source file, never recorded as acceptable. The evidence
  plan lists each such owner file as a concrete sharedPath with the intended
  edit, and the owning plan's tests must keep passing. A defect outside those
  listed paths goes back to its owning accepted plan by selective redispatch
  (15.3.8.10).

**Closeout** (evidence plan):

- Move the 15 `impl-plans/active/canvas-editor-224-*.md` files, plus
  `canvas-editor-224-dispatch.json`, to `impl-plans/completed/`, each listed
  as a concrete path with a one-line superseded note. Do the same for the
  completed `canvas-cutover-*` plans.
- Update `impl-plans/README.md`.
- Record the final gate logs on the closeout commit.

##### 15.3.8.10 Edit-path cost and timing gates (session 269)

**Issue reference:** `workflowInput:RESUME-session-267` (resumed as session
269; findings IR-S267-001 and IR-S267-002). This subsection clarifies how
15.3.8.3, 15.3.8.7 and 15.3.8.8 apply to the edit path. It adds no
dependency, no protocol field and no new surface member.

**Bounded edit-path rule.** These paths are the edit path:

- a document-changing transaction from typing, IME commit, paste of at most
  64 KiB, word or line delete, or undo/redo replay through `CodeSurface`;
- the `DocumentSync.apply` call that follows it;
- the syntax-provider work on the next text-dirty frame;
- every animation-only frame.

On the edit path, no code calls `Text.toString()` on the document, and no
`Text.sliceString` call, string `slice` or chunk read covers more than 64 KiB.
Text read is proportional to the changed lines plus the visible range, not to
the document. Numeric per-line bookkeeping may be linear in the line count. The proof is operation counters in jsdom
(`editor/test/canvas/edit-cost.test.ts` and the mounted case in
`editor/test/canvas/mount.test.ts`) on the 20,000-line document. These tests
use no wall-clock assertion.

Whole-document reads stay allowed only off the edit path:

- the whole-text transfers of 15.3.8.7 (open/reset, song apply, format,
  completion request, debounced local check, eval);
- save;
- building a `Utf8Index` to map a wire span of a revision that a received
  message references, cached by `RevisionHistory` as today.

**Document sync (`code/sync.ts`, `code/history.ts`).**

- `DocumentSync.apply` converts UTF-16 changes to byte changes and dirty
  spans without `RevisionHistory.index` and without a whole-text
  `Utf8Index`.
- The sync keeps the per-line UTF-8 byte lengths of the current revision, plus
  a prefix structure over them. The byte offset of a position is the bytes
  of the lines before it plus the bytes of its line prefix. The line prefix
  is read in chunks of at most 64 KiB.
- Each applied change set updates only the lines it touches.
- When the transaction's start document is not the recorded current text (a
  history reset or revision gap), the table is rebuilt from that document's
  lines. This is linear in the line count and never calls `toString`.
- Undo and redo are ordinary surface transactions, so they take the same
  path.
- The `doc-changed` bytes, dirty spans, revisions, epochs and
  `mapWireSpan`/`toWireSpan` results are byte-identical to the current
  implementation. Tests compare against a reference `Utf8Index` on
  documents with ASCII, Japanese, emoji (surrogate pairs) and CRLF content.
- The `CodeSurface`, `DocumentSync` and `RevisionHistory` public signatures
  that the accepted MOUNT and consumer code use do not change. Making
  history internals line-aware is allowed.

**Syntax provider (`code/syntax.ts`, `code/syntax-core.ts`).**

- Change detection uses the `Text` identity plus the change sets reported
  through `noteChanges`. String comparison is not used.
- **Tree-sitter.** `parse` and `reparse` read through web-tree-sitter's
  `ParseCallback` (pinned 0.27.0), served from `Text.sliceString` in chunks
  of at most 64 KiB. Edit points come from `Text.lineAt`
  (`row = line.number - 1`, `column = pos - line.from`), in the same index
  units as today. Captures use `query.captures(root, { startIndex, endIndex })`
  for the visible range plus overscan. The first parse after mount or reset
  reads the whole document in chunks; that read is off the edit path.
- **Fallback tokenizer.** As 15.3.8.2(c) already requires: per-line start
  state is cached up to the viewport end, truncated at the first changed
  line, and only visible lines plus overscan are tokenized per text phase.
- Span output, the 16,384-span cap, `syntax-truncated` and
  `codePane.dataset.syntax` stay as specified.

**Wall-clock vitest gates.** Superseded by 15.3.8.11 (session 271). The
budget rule (2x the median of three focused runs on the final source, rounded
up to 100 ms, recorded in the plan; never raised past 2x; no harness timeout
widened to hide a slow product path) is kept and now applies only inside the
serial perf gate.

**Mutation evidence.** Mutation and negative-control runs (the fix reverted
in a scratch copy or through a test-only toggle that is off by default) must
exit nonzero. They are reported with log paths, separately from the gating
list, and never gate acceptance.

**Selective redispatch.** If wave-4 measurement finds a product defect in a
file that is not on the evidence plan's pre-listed seam paths, it goes back
to the owning accepted plan by selective redispatch, with a concrete
writePaths amendment and a changed manifest fingerprint. It is never
recorded as acceptable. Accepted plans without such a finding are not
redispatched.

##### 15.3.8.11 Serial perf gate (session 271, operator decision P)

**Issue reference:** `workflowInput:RESUME-session-269` (resumed as session
271). Session 269 found that the absolute 4,800 ms budget in
`editor/test/e2e/large-eval.test.ts` passes or fails unpredictably in the
default parallel vitest run: default-worker full runs exited 0, 1, 1, with
20,000-line medians of 4,924.1 and 5,685.6 ms. Three `--maxWorkers=1` full
runs passed 696/696. Only test files and test configuration change here. No
product source, protocol, dependency or accepted plan scope changes.

**Rule.** The default suite (`npm run test`, which is `vitest run`, and
`./node_modules/.bin/vitest run`) contains no assertion on measured elapsed
time. Every absolute wall-clock budget lives in a perf-only file that runs only
in the serial perf gate. No assertion is skipped, deleted or made conditional
inside a file. Test timeouts (the third `it` argument) are harness bounds, not
budgets, and stay where they are.

**Audit (2026-10-05, at 6dc176f).** `grep -rnE "performance\.now\(\)|Date\.now\(\)|hrtime" editor/test`
over the `.ts`/`.tsx` tests finds exactly one elapsed-time assertion:
`large-eval.test.ts:53` (`largeMedian <= LARGE_EVAL_BUDGET_MS`). The other
hits are not budgets. `test/app/main.test.ts` passes `performance.now()` into
the audible clock as a sample time. The `Client` in `test/code/sync.test.ts`,
`test/canvas/{state,edit-cost,input}.test.ts` uses `Date.now` as its clock.
`large-doc.test.ts` asserts bytes and has a 60,000 ms test timeout.
`first-viewport.test.ts`, `frame.test.ts` and the other `test/canvas/*` tests
use injected or simulated clocks. The RUNSTART plan repeats this audit on the
final source and records the result. Any new hit moves under the same rule.

**Selection mechanism (fixed, so plans do not choose).**

- Perf-only files are named `editor/test/**/*.perf.test.ts`. The first one is
  `editor/test/e2e/large-eval.perf.test.ts`.
- `editor/vitest.config.ts` reads `process.env.VACTR_PERF`. When it is `1`,
  `test.include` is only `test/**/*.perf.test.ts`. Otherwise `test.include`
  stays as it is and `test.exclude` adds `test/**/*.perf.test.ts` to the
  vitest defaults (`configDefaults.exclude`). The two sets never overlap.
- `editor/package.json` adds
  `"test:perf": "VACTR_PERF=1 vitest run --maxWorkers=1"`. The installed
  vitest 5.0.2 accepts `--maxWorkers`; session 269 used it. The `test` and
  `e2e` scripts do not change.
- The perf gate runs alone. No other vitest, nextest, cargo build or browser
  run is active on the host at the same time. A failed perf run is a failure.
  It is not retried until it passes.

**`large-eval` split.** The measurement helper (fresh `session_init`, one
`eval` envelope, `performance.now()` around `session_apply`, median of three)
may move into a non-test module, `editor/test/e2e/large-eval-shared.ts`. That
name does not match `*.test.ts`, so vitest never collects it as a test.

- `large-eval.test.ts` (default suite) keeps: one `eval-result` per run, zero
  error diagnostics for the 20,000-line run, and the 20,000/5,000 median
  ratio. The ratio limit drops from 8 to 6. Fixed source measured 2.93
  (focused). The pre-fix Wasm mutation run measured 6.87
  (`s269-mutation-prefix.log`: 20,566.3 and 141,240.7 ms), so 6 still fails
  the pre-fix shape. A ratio of two medians taken in the same worker is
  insensitive to contention. The default file has no absolute ms value.
- `large-eval.perf.test.ts` (perf gate) asserts the same one-result and
  zero-error checks, and the 20,000-line median at or below the absolute
  budget. It also keeps the ratio at or below 6.
- **Budget.** 2x the median of three focused serial runs of the perf file on
  the final source, rounded up to 100 ms. The session-269 value was
  m = 2,372.8 ms, so 4,800 ms. If the final source is unchanged, the plan may
  keep 4,800 ms and must record that it re-measured and confirmed it. Either
  way, `canvas-cutover-evidence-runstart.md` records the three medians and the
  budget. The budget is never raised past 2x.
- **Negative control** (reported separately, never gating): against the
  pre-fix scratch Wasm (15.3.8.10 mutation evidence), each of the two files
  exits nonzero. The log names the failing assertion or the test timeout.

**Ownership.** RUNSTART owns these concrete paths:

- `editor/test/e2e/large-eval.test.ts`;
- `editor/test/e2e/large-eval.perf.test.ts` (new);
- `editor/test/e2e/large-eval-shared.ts` (new, optional);
- `editor/vitest.config.ts`;
- `editor/package.json` (the `scripts.test:perf` entry only).

`editor/package.json` stays a sharedPath of the evidence plan for
`scripts.e2e`. RUNSTART lands first, and the evidence plan does not edit
`test:perf`. EDITCOST's scope does not change.

**Gates.** These replace the three-consecutive-run sentence of the old
15.3.8.10 paragraph. EDITCOST and RUNSTART are accepted only when all of the
following hold on the joined tree:

- the default `./node_modules/.bin/vitest run` exits 0 three consecutive times
  with default workers;
- `cd editor && npm run test:perf` exits 0;
- the other 15.3.8.9 "green after every plan" commands pass.

The final closeout gates of 15.3.8.9 add `npm run test:perf`.

## 16. Wasm and AudioWorklet Layout

No SharedArrayBuffer, no COOP/COEP (decided). Two instantiations of the
one `wasm32-unknown-unknown` module:

```
main thread:  wasm #1  evaluator+scheduler+checker  ── postMessage ──►  AudioWorklet
              TS shell (editor, WebGL render, MIDI/OSC via WebMIDI/WS)      wasm #2: DSP only
```

- **Main-thread instance** runs everything in sections 5–11 and 13–14.
- **Worklet instance** runs only the `dsp` module. It receives graph
  templates, sample data, and event batches via `port.postMessage`
  with transferables; no shared memory. `MessagePort` handlers execute
  ON the audio rendering thread (AudioWorkletGlobalScope), so the
  no-alloc/no-block guarantee covers `process()` by construction and
  the handlers by the bounded-work protocol of 16.1 — moving work "off
  the render quantum" does NOT move it off the rendering thread, and
  the design does not pretend it does.
- **Clock**: the worklet is the timebase. Each render quantum it posts
  `currentFrame / sampleRate`; the main thread's scheduler runs AHEAD
  by the latency window and ships timestamped events, so message jitter
  is absorbed (SuperCollider latency model). Events arriving late are
  played immediately and counted; the session raises the latency window
  when late events exceed a threshold, and reports it.
- Native builds swap the same seams: cpal callback instead of the
  worklet, a ring buffer instead of postMessage; `dsp` code is
  identical (it sees only `&mut [f32]` buffers and POD events).

### 16.1 Worklet resource lifecycle (browser)

- **Preallocation.** The worklet Wasm instance allocates everything at
  init: a fixed-capacity sample arena (default 64 MB, configurable
  before the AudioContext starts), fixed graph-template slots, the
  voice pool, and both rings. Wasm memory never grows after init;
  changing capacities is an explicit audio restart the user confirms.
- **Windowed install protocol (sender-paced).** Decoding and
  normalization happen OFF the audio thread (main thread or a plain
  Worker). Transfers are sliced (64 KB): the main thread posts ONE
  slice; the worklet copies it into the preallocated arena WITHIN its
  per-`process()` credit (below) and acks `SliceOk` back over the port
  only AFTER the copy has actually run — a slice arriving with the
  quantum's credit spent is deferred to the next `process()` and its
  ack withheld until then; the MAIN THREAD posts the next slice only
  on that ack. The continuation is sender-driven over the existing
  entangled port pair — a `MessagePort` posts to its peer, not to
  itself, so no self-posted-message route is assumed —
  and the in-flight window is one slice, so a BURST of install
  requests queues on the main thread and the rendering thread never
  performs more than one bounded copy per round trip, however many
  installs are pending. Admission is checked before sending: a
  request larger than the free arena space fails immediately with a
  host diagnostic. When the last slice is acked the worklet completes
  the resource and acks `Installed { resource, gen }`; the scheduler
  references a resource only after that ack.
- **Uniform generation-aware retirement — samples AND graphs.** Every
  installed resource (sample bank, graph template) has a resource id;
  the audio side refcounts its users: queued events referencing it,
  active voices, and graph templates referencing sample banks.
  `unload` (or graph replacement) marks the resource RETIRING: the
  scheduler immediately stops emitting new references; arena blocks
  and template slots return to the free list ONLY when the audio side
  reports the refcount at zero via a `Retired { resource }` message.
  Nothing is reclaimed while a queued event or a sounding voice can
  still read it, so a later install can never overwrite in-use
  storage. Only POD bytes and indices cross the boundary — no Rust
  object is shared between the two isolated Wasm memories (the native
  triple-buffer / `Arc` scheme of 12.2 does not apply in the
  browser).
- **Aggregate work bound tied to audio progress, not to
  acknowledgment concurrency.** Single-flight admission alone does NOT
  bound work per render interval — several immediate `SliceOk` round
  trips can complete within one quantum, so bounding "one unacked
  slice" is insufficient (Astra's counterexample). The worklet
  therefore enforces an audio-progress CREDIT budget: it maintains a
  copy budget of at most `INSTALL_BYTES_PER_QUANTUM` (default 64 KB,
  i.e. one slice) of install copying PER `process()` call, and
  replenishes credit only when `process()` runs (i.e. as audio
  actually advances), never on wall-clock or on ack arrival. A slice
  message that arrives with the current quantum's credit already
  spent is not copied immediately; its copy is DEFERRED to the next
  `process()` boundary and the `SliceOk` ack is withheld until the
  copy completes, so the sender's single-flight window plus the
  per-quantum credit together bound aggregate rendering-thread copy
  work to one slice per audio quantum regardless of how fast acks
  return. **Backpressure/overload**: because credit gates on audio
  progress, a burst simply installs at one slice per quantum — slower,
  never louder in CPU — and the session surfaces install latency; if
  the deferred-slice queue exceeds a bound the install fails with a
  host diagnostic rather than growing unboundedly. Graph templates
  are separately bounded by construction: `inst` definitions cap node
  count (default 256) at definition time, keeping any template within
  one slice; an oversized definition is a compile-time diagnostic.
  This is the application-controlled guarantee; WHEN the browser
  schedules `process()` remains browser-managed, but the budget is
  now expressed in the browser's own unit of audio progress, so it
  holds whatever the ack timing.
- **Planned browser verification (TASK-008, in its own standalone dev
  harness — see the plan):** sample-bank load and graph replacement
  during active playback without audible dropout; arena-exhaustion,
  oversized-graph, and deferred-queue-overflow diagnostics;
  unload-while-playing holds storage until queued events and voices
  release it; an install burst installs at MOST
  `INSTALL_BYTES_PER_QUANTUM` per `process()` call — measured as total
  bytes copied on the rendering thread per quantum across the whole
  burst, NOT merely unacked-slice count — with acks withheld until
  the deferred copy runs; the headless `process()` allocation test
  stays separate — passing it alone does not establish the messaging
  guarantees.

## 17. Concurrency Invariants and Security Boundaries

Invariants (enforced by types where possible):

1. All user code on ONE evaluator thread; `Value` is `!Send`.
2. Audio and render threads receive values only — POD events, graph
   templates, `f32` uniforms, and render-safe descriptors (shader
   text plus plain metadata, 9.2); never a `Value`, a `VParam`, or a
   closure.
3. No allocation, locking, or `Rc` traffic in the audio callback; in
   the browser, worklet message handlers (also on the rendering
   thread) do only bounded copies into preallocated targets (16.1).
4. Pattern query is pure, ENFORCED by the Query effect mode (10.4).
5. Every failure carries origin (span, slot, beat); a live session
   never dies from a user error.
6. Logical time is `Ratio64` everywhere inside; `f64` seconds only at
   host boundaries.

Security boundaries: user code reaches the outside world only through
capability traits — no filesystem or network primitives in the
language; package content crosses the filesystem boundary only
through the validated, contained, atomically published pipeline of
5.7 (path normalization and containment, no links/devices, bounded
extraction, digest-verified staging before cache publication);
`SampleLoader` reads only configured sample directories/URLs;
OSC/MIDI sinks send only to user-configured destinations. In the
browser everything sits in the Wasm/JS sandbox. The Tauri shell keeps
the minimal allowlist: file dialog scoped read/write for `.vact` and
sample folders, no shell execution, no arbitrary fs scope. The session
WebSocket binds to localhost with a random token printed by the CLI.
No secrets are stored; editor binding files contain controller mappings
only.

## 18. Failure Handling, Rollout, Rollback

- Bind-time: dry run against `NoopHost`; failure = diagnostic, old
  pattern keeps playing (retry = user re-evals).
- Event-time: failing event dropped + reported; slot continues;
  diagnostic cleared on next successful cycle of that slot.
- Handoff: ring-buffer full = event dropped + counted + reported as a
  host diagnostic (never blocks the evaluator or audio thread).
- Worklet lateness: absorbed by latency window; auto-widen + report.
- Rollout: milestones follow the implementation plan order (language +
  REPL, then scheduler/DSP, then editor); each milestone is
  independently runnable (REPL plays sound before any editor exists).
  Rollback of any milestone is `git revert` of its tasks — no data
  migration exists; editor binding files are versioned JSON with a
  `v` field, ignored-if-unknown.

## 19. Alternatives Considered

| Choice made | Alternative | Why not |
|-------------|-------------|---------|
| `Value` enum, `Rc`, `!Send` | NaN-boxing / 64-bit tagged words | measurable only in hot arithmetic; enum is safer, wasm-friendly, and control-rate workloads are small; revisit behind the same `Value` API if profiles demand |
| Immutable `Rc` lists with literal provenance | persistent vector (`im`/RRB) | live-code lists are short; `put` copy is fine at control rate; swap-in later without API change |
| Pattern node tree | patterns as opaque Rust closures (Tidal-style) | closures cannot be inspected for dry-run explanation, step provenance (highlighting), or tweak sites; tree costs one match dispatch per node |
| Bytecode VM from the start | tree-walking interpreter v1 | the VM is mandatory anyway (iOS, decided); a tree-walker would be a second engine to keep in sync; the compiler is small because the kernel is 8 forms |
| Two wasm instances, postMessage | SAB + shared-memory ring | SAB needs COOP/COEP (decided against); latency-window model is proven by SuperCollider/Strudel |
| Editor speaks session protocol; LSP for external editors | LSP-only for everything | LSP has no vocabulary for telemetry, tweaks, transport; both share the checker so there is no duplication |
| Tauri wraps the same Wasm frontend (v1) | native core in Tauri via IPC from day one | one code path first (web-first, decided); the socket seam already exists for the low-latency upgrade |
| Hand-rolled `Ratio64` | `num-rational` | overflow-as-failure and int64/int64 fixed width are language semantics; a dependency would be wrapped anyway |

## 20. Open Questions — Recommendations

**These four began as recommendations only.** Each respects every
Decided item and is reversible before implementation of the affected
module. **Status (author, issue #2, 2026-09-25):** all four are
decided. Q1 is Decided as the parent-scope model (the recommendation
below, which superseded an earlier strict decision the same day); Q2
is answered by M1 (follow the recommendation for the fixed set in
7.1.4, `design-docs/user-qa/pending-middle-end-questions.md`); Q3 is
Decided as `grid`; Q4 is Decided as both names.

**Q1 — Shadowing rule versus the global prelude.** *Decided
(author, 2026-09-25, SUPERSEDES the earlier strict decision): the
recommendation below is ADOPTED. Scope chain prelude -> session -> fn/block;
no rebinding within one scope; a child scope may shadow a parent binding
(prelude shadow = hint, user-parent shadow = warning); the prelude is
read-only. Motivation: the builtin `sound-kit` is overridden by binding
`sound-kit` in the session (design-music.md, sound kits). Also decided:
`path` and `url` literal types and `load path` (lang-reference.md).
Implementation: 5.6 scope model, 7.1.4, 6.5.8.*
*Recommendation (adopted):* move the prelude to a parent scope of the
session namespace, so a user binding shadows the prelude name with an
LSP diagnostic ("shadows prelude `scale`", a hint as decided) and
one-keystroke rename support; "no shadowing" then holds within any
single scope only. Rationale: strict prohibition makes hundreds of
short prelude names (`scale`, `range`, `time`, `delay`, `line`, …)
unusable as user names mid-performance (principle 3), while silent
shadowing would hide typos; the diagnostic keeps it honest.

**Q2 — Prelude overloading on the subject argument type
(`scale`, `repeat`, `shape`, `add`).** *Answered (M1, 2026-09-25):
adopted for the fixed set of 7.1.4.*
*Recommendation:* allow type-directed overloading for a FIXED prelude
set only, resolved on the subject (first) argument: statically by the
checker when the type is known, dynamically by the subject's runtime
tag in the VM's native-call dispatch otherwise (pattern vs texture vs
list vs number). User code cannot define overloads in v1. This keeps
each domain's familiar names (Tidal `add`, Hydra `shape`) without
renaming, at the cost of one tag switch in ~6 native functions; the
alternative (renaming one side) was already taken once where collision
was rare (`tile` for Hydra `repeat`) and should stay the exception.

**Q3 — Rename of Tidal's `struct` pattern function** (collides with the
`struct` keyword). *Decided (2026-09-25): `grid`.*
*Recommendation:* rename to **`grid`** (`grid {s :bd} [true false true
true]`): plain-English, subject-first, evokes placing onsets onto a
boolean grid, and is unclaimed across all three vocabulary tables.
Alternatives considered: `steps` (reads well but likely wanted for a
step-count utility), `sieve` (implies removal, wrong direction),
`trig` (SuperCollider ugen name, wrong domain). The design reserves
`PatNode::Grid` accordingly; renaming later is a one-word change.

**Q4 — `amp` versus `gain`.** *Decided (2026-09-25): both stay.*
*Recommendation:* keep BOTH, each in its home vocabulary, with the
documented mapping: `gain` is the pattern CONTROL (Tidal/Strudel name,
what performers type in chains), `amp` is the instrument PARAMETER
(SuperCollider/Overtone name in `inst` headers); the control router
maps `gain` onto `amp` exactly as `design-music.md` already writes
("gain -> amp"). Unifying on one name would break one community's
muscle memory for no mechanical gain — the mapping is one row in the
control table (11.4).

## 21. References

- `design-docs/specs/lang-reference.md` — core language (authoritative).
- `design-docs/specs/design-music.md`, `design-visual.md` — domains.
- `design-docs/specs/architecture.md` — decision log, threads, targets,
  realtime validation (authoritative).
- `design-docs/specs/notes.md` — deferred music-visual coupling; Rhombus findings.
- `design-docs/references/README.md` — external references index.
- Implementation plan: `impl-plans/active/vactr-core.md`.
