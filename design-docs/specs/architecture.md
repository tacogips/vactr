# Architecture Design

This document describes system architecture and design decisions.

## Overview

vactrol is a live-coding scripting language and its LSP server, implemented in Rust.
The surface syntax is indent-based and desugars into S-expressions; the core is a
small Lisp. It runs as an interpreter first and is designed so that a JIT and a
Wasm backend can be added without changing the language.

Design priorities, in order: simplicity, ease of writing, then everything else.

---

## Sections

### Decision Log

| Date | Topic | Decision | Status |
|------|-------|----------|--------|
| 2026-09-24 | Host platform | Own runtime in Rust. Not built on Racket/Rhombus (see `notes.md`). | Decided |
| 2026-09-24 | Live-coding domains | Both music/audio and visuals/graphics are first-class. | Decided |
| 2026-09-24 | Wasm / browser | Definite target. Runtime must stay `wasm32`-compilable. | Decided |
| 2026-09-24 | Typing | Statically typed with inference (principle 4). Annotations optional, Python-style in headers/bindings/fields. Type errors: diagnostics in Live, compile errors in Frozen. `any` explicit. | Decided |
| 2026-09-24 | Indent rule set | Deferred. See `user-qa/pending-indent-syntax.md`. | Pending |
| 2026-09-24 | Macro hygiene | Unhygienic with auto-gensym (`x#`) proposed; hygiene later if needed. | Proposed |
| 2026-09-24 | Collections | Clojure-style literals (`[]`, `{}`, `:kw`), immutable by default proposed. | Proposed |
| 2026-09-24 | Principle 2 | Syntax flows left to right; only `if`, `match`, and `{}` groups require planning ahead. Library functions take their subject first. | Decided |
| 2026-09-24 | Nesting | `{}` is the only nesting form; `( )` is removed from the surface syntax. | Decided |
| 2026-09-24 | Blocks | `{}` and `:`+indent are the same block; strict vs block parameters (lang-reference QA-54). | Proposed |
| 2026-09-24 | Pairs, dict, struct | `key: value` is sugar for the pair `[:key value]`; a dict IS a list of pairs (`[amp: 0.5 pan: -1]`); `{}` is only ever a block. `struct` = named, closed pair-list with defaults; an enum variant is a struct. Named arguments are pairs. Runtime: a sorted map (B-tree), key-ordered iteration; the pair list is syntax only. | Decided |
| 2026-09-24 | Core vocabulary | Plain-English names, not Clojure's: `put` (add/set/merge), `join`, `len`, `first`/`last`/`tail`, `print`, `enumerate`, `repeat value count` (subject first). Domain names stay Tidal/SuperCollider/Hydra; Hydra `repeat` -> `tile`. Lists are callable with an index. | Decided |
| 2026-09-24 | Patterns, one language | `->` marks a pattern that can fail. Binding forms (`let`, `for`) put the name first and take irrefutable patterns; testing forms (`match`, `if`) put the subject first and write `pattern -> body`. Guards only in `match`. `each` withdrawn. | Decided |
| 2026-09-24 | Pattern matching | `match` with literals, arrays, dicts, guards, or-patterns, and data-carrying enum variants. | Proposed |
| 2026-09-24 | `if` | Sugar over `match` (`false \| nil` clause = else); `elif` likewise; `when` and `unless` withdrawn (`if` without else; `if {not c}`). | Decided |
| 2026-09-24 | Kernel | Core forms: `match`, `for`/`break`, `fn`, `let`, `->`, `>:`, `{}`, `enum`. Everything else is a function or sugar. | Proposed |
| 2026-09-24 | Binding `if` | Zig-style capture: `if subj pattern -> body`; the same truthiness as plain `if`. Optionals are nil-able values. | Decided |
| 2026-09-24 | `while`, `loop`, `break` | Withdrawn (principle 5). Iteration is `map`/`filter`/`reduce`/`find`/`take`; `for` is sugar over `map`; `0..` is a lazy infinite range consumed by `take`. | Decided |
| 2026-09-24 | Identifiers | `[a-zA-Z][a-zA-Z0-9]*(-[a-zA-Z0-9]+)*`: no `_`, no leading digit or `-`, ASCII only. `_` and `_<digits>` are language tokens; operators are a separate non-user-definable class. | Decided |
| 2026-09-24 | Errors | No error type in the language. A failure unwinds to the top level or the current pattern event, is reported with origin (source, loop, beat), and the session continues. No try/catch, no user-raised errors in v1. nil = absent (`?T`, `x ? d`). `x ! d` optional fallback proposed. | Decided |
| 2026-09-24 | nil (Clojure) | Option is nil (`?T` = T or nil, no wrapper). nil and false are falsy, all else truthy; one rule for plain and binding `if`/`while`. nil-punning on accessors; `and`/`or` return values; `x ? d` == `or x d`. | Decided |
| 2026-09-24 | Pipe | `>` is the pipe (thread-first); a line beginning with `>` continues the previous expression. The `>:` block form is withdrawn. Head-position `>` is greater-than. | Decided |
| 2026-09-24 | `_1` | Console only; a reader error in files. | Decided |
| 2026-09-24 | let / var / upd | `let` immutable, `var` mutable, `upd` reassigns a var. No shadowing in any direction. Values immutable; vars are rebound. Top-level var and fn are late-bound; patterns read them at each event. | Decided |
| 2026-09-24 | Named arguments | `name: value` pairs at call sites; keyword parameters with `= default` in fn headers. | Decided |
| 2026-09-24 | Numbers | Fixed widths: `int` i32, `int64`, `float` f32, `float64`, `ratio` int64/int64. Literals adapt to context. Implicit widening only; explicit narrowing; overflow fails. | Decided |
| 2026-09-24 | Strings | Only for text a human reads or sees (messages, on-screen text, paths, OSC addresses, shader source). Never for names or notation. `{}` interpolation proposed. | Decided |
| 2026-09-24 | Sequences | No string mini-notation. A list where a pattern is expected is one cycle of steps (nested = subdivide, nil = rest); steps are typed keywords or numbers; `alt`, `maybe`, `euclid`, `hold`, `rep`, `choose`, `fast`, `stack` replace the operators. | Decided |
| 2026-09-24 | Kernel (revised) | `match`, `fn`, `let`, `var`, `upd`, `->`, `{}`, `enum`; `if`/`elif` sugar over `match`, `for` sugar over `map`; no loop form, no `break`. | Proposed |
| 2026-09-24 | Principle 3 | Readable live code over type safety: no rule may lengthen a chain for a static guarantee. | Decided |
| 2026-09-24 | Principle 5 | Declarative: time is data; no sleep/wait/tick. Patterns are the only event source; instruments and visuals are chains; names are late-bound. | Decided |
| 2026-09-24 | Threads and effects | Declarative but not pure: `var`/`upd`, `print`, `once` allowed. All user code on one evaluator thread; audio and render threads receive values only. Iterators (`next`) and `on-trigger` callbacks withdrawn in favour of lazy sequences and the `hits` event signal. | Decided |
| 2026-09-24 | Music into visuals | Deferred (author: specify music and visuals separately for now). Draft parked in `notes.md`. | Deferred |
| 2026-09-24 | Specification split | `lang-reference.md` = core language; `design-music.md` = time, sound, patterns; `design-visual.md` = Hydra chains. | Decided |
| 2026-09-24 | Targets | Browser (Wasm), macOS, iPad, iPhone, other desktops. The bytecode VM is the mandatory engine (iOS forbids JIT); JIT is desktop-only. DSP in the AudioWorklet without SharedArrayBuffer. | Decided |
| 2026-09-24 | Implementation language | Rust core; Swift for the Apple shell only (one SwiftUI app for macOS/iPadOS/iOS via UniFFI bindings); TS shell for the browser. Swift-for-the-core, Zig, C++/JUCE, TypeScript evaluated. | Decided |
| 2026-09-24 | Model | TidalCycles/Strudel (patterns) + Overtone (instruments as ugen chains) + Hydra (visual chains). The Sonic Pi imperative layer (live-loop, sleep, sync, cue, density, tick, play/sample-now, with-fx blocks, p5 draw loop) is withdrawn. | Decided |

### Execution Modes

Live coding requires runtime redefinition; Wasm and JIT want a closed program.
Rather than choosing one, vactrol defines two modes over the same core language.

| Mode | Purpose | `eval` / REPL | Redefinition | Backend |
|------|---------|---------------|--------------|---------|
| Live (default) | Interactive sessions, hot reload | Available | Cheap, per-var | Dynamically checked VM; the static checker is a diagnostic layer |
| Frozen | Distribution, browser bundles | Not available | None | Typed IR to Wasm or native |

Rule: no core-language feature may make Frozen mode impossible. Features that
only exist in Live mode (e.g. `eval`) live in the runtime, not in the language core.

### Pipeline

```
source text (indent syntax)
  -> reader        : indent rules -> S-expression AST   (also used by LSP/formatter)
  -> expander      : macros -> core forms               (compile time only)
  -> evaluator     : v1 tree-walking, then bytecode VM, then JIT on hot paths
  -> (frozen only) : core forms -> typed IR -> Wasm / native
```

The S-expression AST is the single shared data model: the LSP, formatter,
macros, and both execution modes all consume it. No stage below the reader
sees indentation.

### Redefinition Model

Top-level definitions live in namespaces as late-bound vars (Clojure-style
indirection). Redefining `foo` replaces the var's value; existing callers
observe the new value on their next call. This is what makes live redefinition
cheap and safe, and it costs one pointer load per global call, which the JIT can
remove in Frozen mode.

Errors raised by an expression are reported and discarded; the session and any
running schedule continue. A live session must never die from a user error.

### Time Model

Time is data (principle 5). There is no `sleep`, no per-loop thread, and
nothing that suspends: patterns (TidalCycles model) are the only event
source, bound to slots; instruments are unit-generator chains (Overtone
model); visuals are texture chains (Hydra model). The scheduler queries
the bound patterns cycle by cycle and realizes their events.

```
core: cycle clock (bpm, beats per cycle) + slot table + pattern query
  |-- audio capability   : instruments (ugen chains), samples, MIDI, OSC
  |-- graphics capability: texture chains, outputs o0..o3, fft/amp signals
```

Logical time is exact: beats, step lengths, and cycle positions are
`ratio` values (int64/int64) throughout the scheduler and the pattern
engine, as Tidal uses `Rational`. A subdivided sequence lands on exact
positions. Conversion to seconds (`float64`) happens once, at the host
boundary (audio callback, frame clock).

Top-level `var` and `fn` are late-bound: a pattern that names them reads
the current value at each event, so `upd` and redefinition are heard at
the next event without re-binding. Re-binding a slot switches at the
next cycle boundary.

Because no body suspends, the VM needs no coroutines, the browser build
needs no threads, and a dry run of a pattern (Realtime Validation) is a
complete check of it.

Capabilities are traits implemented by the host (native binary or browser).
The core has no direct dependency on any audio or graphics library.

### Threads

All user code runs on one evaluator thread (author, 2026-09-24:
"declarative, but not obsessed with immutability; side effects are
allowed"). That single rule makes `var`/`upd`, `print`, and `once` safe
by construction, exactly as in JavaScript, Hydra, and Strudel.

| Thread | Runs | User code |
|--------|------|-----------|
| Evaluator | REPL evaluation, pattern queries per cycle, control-rate signal values, late-bound `var`/`fn` reads, LSP checks | all of it |
| Audio (real-time callback; AudioWorklet in the browser) | the DSP graph that `inst` chains compile to; sample playback | never; receives events ahead of time through a lock-free queue (SuperCollider's latency model) and control-rate values (control buses) |
| Render (GPU, per frame) | the shader a visual chain compiles to | never; receives uniforms (`time`, `fft`, `amp`, `beat`, `hits`, signal parameters) |
| I/O | editor/LSP transport, MIDI, OSC | no |

Invariant: a vactrol closure is never evaluated on the audio or render
thread. A function-of-time parameter is evaluated at control rate on
the evaluator thread and pushed as a value. Visuals therefore need no
multithreading of user code; the GPU is the parallelism. In Wasm the
same layout is main thread plus AudioWorklet, with no shared memory.

### Targets and Implementation Language (DECIDED, 2026-09-24)

Targets: browser (Wasm), macOS, iPad, iPhone, other desktops. Consequences:

- **The bytecode VM is the mandatory execution engine.** iOS forbids JIT
  outside WebKit, so a native iOS app can never use Cranelift; the JIT
  is a desktop-only optimization, never a requirement. Frozen mode is
  AOT and unaffected.
- **DSP runs inside the AudioWorklet in the browser**, with the Wasm
  module instantiated in the worklet itself, so no SharedArrayBuffer and
  no COOP/COEP headers (Safari needs them for SAB).
- **The DSP graph never allocates or blocks** on the audio thread; the
  implementation language must have no GC there.

Author's confirmation (2026-09-24): "one Rust core -- reader, checker,
VM, scheduler, pattern engine, DSP graph, and the LSP sharing the
checker -- built three ways; deliver web-first, Swift/CoreAudio shell
later; no CoreAudio glue before the language itself works."

Implementation language: **Rust**, one core (reader, checker, VM,
scheduler, pattern engine, DSP graph, LSP sharing the checker) built
three ways:

| Build | Delivery |
|-------|----------|
| `wasm32` + thin TypeScript shell (Web Audio, WebGL) | browser, iPad Safari / PWA; the first deliverable |
| static library for Apple targets + ONE SwiftUI shell (AVAudioEngine, CoreMIDI, later an AUv3 extension); Swift bindings generated by UniFFI | macOS, iPadOS, iOS from one shell; lower latency; the second deliverable |
| native binary (`cpal`) | desktop development build on every OS, and the shipped build for Windows and Linux |

Alternatives evaluated: Swift for the CORE (unbeatable for the Apple shell,
which is where it is used, but its wasm32 support is experimental with
no AudioWorklet path, ARC and copy-on-write can allocate or lock inside
a render callback with no compiler guarantee, and non-Apple platforms
would stay second-class); Zig (matches the language's taste and cross-
compiles everywhere, but wasm interop, LSP transport, and audio
bindings would be written from scratch); C++/JUCE (best audio and iOS
ecosystem, worst fit for a memory-safe VM; a possible host layer for
AUv3); TypeScript (cheapest to ship everywhere, as Strudel and Hydra
show, but no native path and the DSP would be Rust-in-Wasm anyway);
Swift, Go, Kotlin/Wasm, Dart (immature Wasm or a GC on the audio path).

### Wasm Constraints

- The runtime crate must build for `wasm32-unknown-unknown` (browser) from the
  start; `wasm32-wasi` is a later addition.
- No assumption of OS threads in the core; concurrency is scheduler-driven.
- All I/O and platform access goes through capability traits.
- Live mode in the browser runs the interpreter/VM compiled to Wasm; Frozen
  mode emits Wasm from vactrol programs and is a later milestone.

### Realtime Validation

Errors such as division by zero are exceptions (see `lang-reference.md`,
errors). Surfacing them before and while code plays is a tooling concern,
not a language one. Three layers, each independent of the next:

| Layer | When | Where | Catches |
|-------|------|-------|---------|
| Static diagnostics | while typing | LSP | literal `/ x 0`, undefined names, unknown sample/synth keywords, `match` missing an enum variant, annotation mismatches |
| Dry run before swap | on eval of a pattern | runtime | any failure raised by one cycle of the pattern queried against a no-op capability host with the current live state; the old pattern keeps playing until the dry run passes |
| Runtime diagnostics | while playing | runtime -> LSP | uncaught failures from the live session, shown on the originating line with slot name and beat, cleared when the next cycle succeeds |

What the language guarantees to make this possible:

- Every error carries its origin: source position, slot name, beat.
- All effects are capability calls (audio, graphics, I/O), so a
  validation host can implement them as no-ops.
- Pattern evaluation is pure: querying a cycle has no side effects.

Consequences for the toolchain: the LSP server and the live session share
one process (or a socket), because runtime diagnostics originate in the
scheduler. `vactrol` therefore runs both; an external editor connects to
the same session the console shows.

### Typing

vactrol is statically typed with inference (principle 4 in
`lang-reference.md`). Every expression has a type the checker can name;
annotations are optional because inference usually makes them
unnecessary. Annotations go in `fn` headers, `let`/`var` bindings,
and struct/enum fields, Python-style (`name: type`, `= default`).

| Mode | Type error |
|------|------------|
| Live | Diagnostic in the editor and console; the code still runs |
| Frozen | Compile error |

`any` is an explicit escape hatch that must be narrowed by a pattern
before use. Numbers have fixed widths: `int` (i32), `int64`, `float` (f32),
`float64`, and exact `ratio` (int64/int64); literals adapt to context.
Widening is implicit (int -> int64, float -> float64, int -> float/float64);
narrowing is an explicit call; overflow is a failure. Keywords are typed like
Zig enum literals against the enum or host set expected at that
position. The inference model (per-function monomorphic vs HM-lite) is
open as QA-92.

---
