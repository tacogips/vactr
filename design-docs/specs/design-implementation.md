# Vactrol Implementation Design

Supporting document to `architecture.md`: how the Vactrol processor is
built. It designs the reader, static checker, bytecode VM, scheduler,
pattern engine, DSP graph, LSP, REPL, the Tauri/web/Wasm editor, visual
feedback, and controller binding, and it specifies every internal data
structure the runtime uses.

This document defines **no language syntax**. The language is specified
by `lang-reference.md`, `design-music.md`, and `design-visual.md`; the
decision log in `architecture.md` is binding. Where this document names
a surface form it is quoting those specifications. The four open
questions at the end carry **recommendations, not decisions** (section 20).

Implementation plan: `impl-plans/active/vactrol-core.md`.

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
  `vactrol.lock`, a cache, and a browser package proxy (section 5.7).
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

- `Cargo.toml` is a single empty package `vactrol` (lib + bin), no
  dependencies. `src/lib.rs` / `src/main.rs` are scaffold stubs. There
  is no code to migrate; compatibility constraints come only from the
  specifications.
- `design-docs/specs/*.md` carry the Decided items this design must
  respect; the indent-syntax question is resolved piecemeal in
  `lang-reference.md` (see `user-qa/pending-indent-syntax.md`,
  superseded).
- Binary/crate name `vactrol`, source extension `.vact` (`command.md`).

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

One core crate `vactrol` (the existing package), feature-gated hosts.
No workspace split in v1; the editor frontend is TypeScript under
`editor/` and consumes the core through wasm-bindgen or the session
socket.

```
src/
  value/     value.rs ratio.rs key.rs dict.rs intern.rs   # sections 5
  reader/    lexer.rs layout.rs sexpr.rs span.rs          # section 6
  expand/    expander.rs                                  # section 6.4
  types/     ty.rs infer.rs diag.rs manifest.rs           # section 7
  ns/        namespace.rs tweak.rs                        # section 8
  compile/   compiler.rs proto.rs                         # section 9
  vm/        ops.rs frame.rs vm.rs fail.rs                # section 9
  pattern/   pat.rs step.rs query.rs combinators.rs signal.rs  # section 10
  tex/       texnode.rs shader.rs uniforms.rs                  # section 9
  clock/     tempo.rs                                     # section 11.1
  sched/     slots.rs scheduler.rs telemetry.rs           # section 11
  dsp/       ugen.rs graph.rs voice.rs sample.rs          # section 12
  host/      caps.rs noop.rs native/ wasm/                # sections 11.5, 16
  session/   protocol.rs session.rs repl.rs bindings.rs   # sections 13, 14
  lsp/       server.rs                                    # section 14.3
editor/      TypeScript: CodeMirror 6 frontend, Tauri shell, worklet JS
```

| Feature flag | Pulls in | Default |
|--------------|----------|---------|
| `host-native` | cpal, midir, tungstenite (session socket) | yes (desktop dev) |
| `host-wasm` | wasm-bindgen, js-sys, web-sys | no (wasm builds) |
| `lsp` | tower-lsp, tokio | no |

The core modules (`value` … `dsp`) have no I/O dependencies and no
assumption of OS threads, keeping `wasm32-unknown-unknown` green from
the first commit.

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
  how `upd cutoff 400` and `fn kick-sound: :bd-tek` are heard at the
  next event with no re-binding.
- Loading a top-level `let` derefs immediately (a snapshot). Re-running
  the `let` line replaces the slot for future evaluations (live
  redefinition), which is neither mutation nor shadowing.
- Locals live in VM frames; `let` locals are single-assignment, `var`
  locals are frame slots `upd` may rewrite. No shadowing in any
  direction is a checker error; at the top level in Live mode,
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
  `note [raised] > s :pluck > d1`, then `upd root 62` recomputes
  `raised` (eager edge on `root`), rebuilds the pattern form that
  eagerly read `raised`, re-binds `d1` at the boundary, and updates
  the display batch — nothing else recomputes.

**Shadowing baseline.** Under the current decisions there is ONE global
scope that includes the prelude and no shadowing in any direction, so a
user top-level definition of a prelude name is a checker error. The
prelude-as-parent-scope relaxation is a PROPOSED AMENDMENT, not the
implemented default — see section 20 Q1.

### 5.7 Packages, imports, and qualified names (Decided scope)

Implements the Decided package system (architecture.md Packages;
lang-reference.md modules): Go-style repository-path imports, git-tag
semver versions, minimal version selection, a lock file, a cache, and
a browser proxy. Packages are Vactrol code plus assets only — no
native extensions — so a package can never reach the audio thread
except through builtins.

```rust
pub struct PackageId(Rc<str>);         // "github.com/owner/name", lowercase path
pub struct PkgManifest {               // vactrol.toml — fields DEFINED here, as the
    package: PackageId,                // spec delegates them to this design
    vactrol: Option<Rc<str>>,          // minimum language version (semver)
    deps: Vec<(PackageId, Rc<str>)>,   // path -> version requirement ("v1.2.0"-style tags)
    assets: Vec<Rc<str>> }             // sample/wavetable directories, relative
pub struct LockEntry { id: PackageId, version: Rc<str>, sha256: [u8; 32] }
pub struct LockFile { entries: Vec<LockEntry> }   // vactrol.lock, sorted by path
pub trait PackageStore {               // host capability — IO side, never evaluator-blocking
    fn resolve(&mut self, roots: &[(PackageId, Rc<str>)]) -> Result<Vec<LockEntry>, HostError>;
    fn fetch(&mut self, e: &LockEntry) -> Result<PkgSources, HostError>; }
pub struct PkgNs { id: PackageId, ns: Namespace }     // one child namespace per package
pub struct ImportBinding { prefix: SymId, pkg: PackageId, open: bool }
```

- **Resolution.** `vactrol get` (CLI) and the editor's import action
  run MINIMAL VERSION SELECTION exactly as Go: collect every
  requirement reachable from the roots, pick the MAXIMUM of the
  MINIMUM required versions per path, write `vactrol.lock` with the
  chosen version and the CANONICAL CONTENT DIGEST below.
  Native `PackageStore`: shallow git fetch of the tag (or a local
  directory), cached under `~/.vactrol/pkg/<path>@<version>`.
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
  default prefix (the last path segment minus a `vactrol-` prefix:
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
    `import ... vactrol-pads` on line 1 makes `pads.warm` readable
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
  The strict no-shadowing rule (5.6, 20 Q1) governs the session
  namespace itself and is unchanged; open-import precedence is the
  spec's own collision-warning model, not shadowing inside a scope.
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

## 7. Static Checker and Inference

HM-lite with let-polymorphism (decided): unification over

```
int int64 float float64 ratio bool string keyword nil  ?T  [T]  [K: V]
fn T… -> V   pattern T   signal   any   + named struct/enum types
```

plus numeric-literal kind variables (a literal adapts to context;
default int/float), keyword literal typing against the enum or host set
expected at that position (`manifest.rs` holds the host's sample,
synth, and control sets so `:bd-haus` completes and `:not-a-sample` is
a diagnostic), `?T` optional flow (`?` supplies the default; using a
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
variant, annotation mismatches, duplicate dict-literal keys, shadowing
violations, effectful calls inside pattern arguments (purity warning,
section 10.4), and unbounded-source iteration where provable.

The checker is one crate module consumed identically by the compiler,
the LSP (section 14.3), and the session's eval path — "LSP sharing the
checker" is literal reuse, not a port.

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
pub struct Pat { node: PatNode, span: Option<Span> }
pub enum PatNode {
    Steps(Box<[Step]>),                        // one cycle; nested = subdivide
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
    Chunk(Rc<Pat>, PParam, Value), Grid(Rc<Pat>, Rc<Pat>),   // Tidal `struct`; name: section 20 Q3
    Euclid(Rc<Pat>, PParam, PParam, PParam),
    Control(KwId, Rc<Pat>, Rc<Pat>),           // gain/lpf/…: value pattern onto subject
    ScaleNotes(KwId, KwId, Rc<Pat>), Chord(…), Voicing(…), Arp(…),
    Segment(Rc<Pat>, PParam), Range(Rc<Pat>, PParam, PParam),
    MidiNotes { channel: Option<u8> },   // live MIDI note input (11.7): yields NO
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
  (10.1): under pure query and dry run it yields no events (future
  input is unknowable; purity is preserved). COMPOSITION is defined
  by an INPUT-LANE partition performed AT BIND TIME: the dry run
  walks the tree from each `MidiNotes` node to the sink and
  classifies every operator on that path —
  - SUPPORTED live operators, applied PER ARRIVING NOTE in tree
    order: control-attaching nodes (`Control`, `ScaleNotes`, chords
    and their kin — they decorate the note event) and per-event
    probabilistic filters (`maybe`, `degrade-by`, `sometimes-by`
    with a per-event closure: the note is kept or dropped using the
    seeded RNG keyed by the node id and the note's arrival sequence
    number, and a kept note passes through the transform). Binding
    `midi-notes > degrade-by 1 > s :pluck > d1` therefore drops
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

`vactrol repl`: line editor over the same `Session`, console transcript
semantics: `_1`, `_2`, … are console-only registers (reader accepts
them only for `FileId::Console`), written only by expressions that
complete. `print` returns its argument. The REPL is the first
deliverable that plays sound: it drives the native host directly.

### 14.3 LSP

`vactrol lsp` (feature `lsp`): tower-lsp server wrapping the SAME
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
| C→S | `eval` | code, file, span (for flash + origin), `doc_revision` |
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

## 15. Editor: Tauri, Web, Wasm — and What Performance Needs

One TypeScript frontend (`editor/`, CodeMirror 6 + Vite), delivered two
ways from the same code:

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
| Package import (`vactrol get`, proxy in the browser), package diagnostics | 5.7 |
| Visual output panes o0..o3 | visual pipeline (9), WebGL2 RenderHost in the TS shell |

The editor may show visual feedback about music (steps, events,
levels); it does not couple the music and visual LANGUAGES — that stays
deferred.

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

**These four are recommendations only, clearly not decisions**; each
respects every Decided item and is reversible before implementation of
the affected module.

**Q1 — Shadowing rule versus the global prelude.**
*Currently binding behavior:* the decisions state "no shadowing, in
either direction" and give v1 one global scope that includes the
prelude. Under them, a user top-level `let scale 2` collides with an
existing binding and is a checker error. That strict behavior is what
TASK-004/005 implement by default.
*Recommendation — explicitly an AMENDMENT to the no-shadowing decision,
requiring adjudication, not a compatible implementation detail:* move
the prelude to a parent scope of the session namespace, so the user
binding shadows the prelude name with an LSP warning ("shadows prelude
`scale`") and one-keystroke rename support; "no shadowing" would then
hold within any single scope only. Rationale: strict prohibition makes
hundreds of short prelude names (`scale`, `range`, `time`, `delay`,
`line`, …) unusable as user names mid-performance (principle 3), while
silent shadowing would hide typos; the warning keeps it honest. Until
adjudicated, the parent-scope behavior stays unimplemented and the
affected plan criteria are conditional on the outcome.

**Q2 — Prelude overloading on the subject argument type
(`scale`, `repeat`, `shape`, `add`).**
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
`struct` keyword).
*Recommendation:* rename to **`grid`** (`grid {s :bd} [true false true
true]`): plain-English, subject-first, evokes placing onsets onto a
boolean grid, and is unclaimed across all three vocabulary tables.
Alternatives considered: `steps` (reads well but likely wanted for a
step-count utility), `sieve` (implies removal, wrong direction),
`trig` (SuperCollider ugen name, wrong domain). The design reserves
`PatNode::Grid` accordingly; renaming later is a one-word change.

**Q4 — `amp` versus `gain`.**
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
- Implementation plan: `impl-plans/active/vactrol-core.md`.
