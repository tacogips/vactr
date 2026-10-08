# Architecture Design

This document describes system architecture and design decisions.

## Overview

vactr is a live-coding scripting language and its LSP server, implemented in Rust.
The surface syntax is indent-based and desugars into S-expressions; the core is a
small Lisp. It runs as an interpreter first and is designed so that a JIT and a
Wasm backend can be added without changing the language.

Design priorities, in order: simplicity, ease of writing, then everything else.

## Product identifier contract

The public product name is **Vactr**. The Rust package, library and executable,
editor package, Tauri application, repository slug, local checkout directory,
configuration and lock filenames, cache directory, environment-variable prefix,
and user-facing documentation use the corresponding `vactr` or `VACTR`
spelling. The source language keeps its existing `.vact` extension and syntax.
References to the physical optical component describe it by function rather
than treating the product name as a component name.

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
| 2026-09-24 | Effects | Builtin effect catalog (dynamics, EQ/filters, delay, reverb, saturation, modulation, lo-fi, resonator, spatial, restoration, utility, analyzers) usable as pattern controls, inst ugens, and bus chains (`bus`, `master`). | Decided |
| 2026-09-24 | Synthesis models | Sampler, analog modeling, digital (FM, phase distortion, additive), wavetable, granular: all definable in code as `inst` chains over builtin ugens, each also a prelude template. | Decided |
| 2026-09-24 | Packages | Go-style GitHub-path imports, git-tag versions, lock file, cache, browser proxy; packages are Vactr code only. Supersedes "no module system in v1". | Decided |
| 2026-09-24 | Editor: sliders and MIDI | Right-pane slider panel over every numeric site (chosen from all candidates), mouse drag on numbers, MIDI in (learn, `cc` signal, note input), MIDI out sink, MIDI clock/transport sync in and out (Link planned). No syntax for any of it. | Decided |
| 2026-09-24 | Host tiers | Browser tier (Wasm core, AudioWorklet, Web MIDI) first; native tier (same core inside the Tauri app on macOS/iOS, CoreAudio/CoreMIDI) second. Features may differ by tier behind capability traits; the language is identical; a missing capability is a diagnostic. Swift is glue only. | Decided |
| 2026-09-24 | Editor: DAW-style parameter editors | Effect/instrument call sites get meaning-matched editors (EQ curve with bands and live spectrum, filter response, dynamics transfer curve, envelope shape, delay taps, sampler waveform, wavetable frames, granular region, LFO shape, XY pad, euclid ring with hits/steps/rotation). Sequences are edited in code only; step grid and piano roll are displays. All write back to the same numeric sites; all MIDI-learnable; nothing in the code. | Decided |
| 2026-09-24 | Reactive dependency graph | Changing a value updates every binding, pattern, slot, and display computed from it, recomputing only the affected part; the reference structure is kept explicitly. Recommended structure: demand-driven incremental graph (Salsa/Adapton family) with static edges from the checker, revision stamps, early cutoff, push notifications to subscribers; signals are the mental model, not the substrate. | Decided (requirement) / Recommended (structure) |
| 2026-09-24 | Directive comments | Control-panel membership, editor kind, ranges, and MIDI mapping are written in `.vact` as one-line comments with a fixed marker on the line they apply to; the language ignores them, the editor validates them. Shape decided: every directive line starts with `#@`, consecutive lines form one block, applies to the nearest preceding statement/block no deeper than the comment (same line = that line). Labels: written in the trailing comment on the line they name (candidate `#@ bass-filter: lpf cc: 74`), `#@ name X` explicit, block-following for bodies; named things (`let`, `fn`, `inst`, `bus`) are labels already; labels are the preferred provenance identity. Vocabulary to be designed. | Decided |
| 2026-09-24 | Model | TidalCycles/Strudel (patterns) + Overtone (instruments as ugen chains) + Hydra (visual chains). The Sonic Pi imperative layer (live-loop, sleep, sync, cue, density, tick, play/sample-now, with-fx blocks, p5 draw loop) is withdrawn. | Decided |
| 2026-09-25 | Middle-end naming and scope (issue #2) | Shadowing stays STRICT against the prelude, and the parent-scope relaxation is not adopted (design 20 Q1). Tidal's `struct` is `grid` (Q3). `gain` (pattern control) and `amp` (instrument parameter) both stay (Q4). Chord qualities are letter-first keywords (`:maj7 :m7 :dom7 :sus4`), never `:7`. Implementation choices: design-implementation 7.1. | Decided |
| 2026-09-25 | Scope, paths, sound kits, sound first (issue #2) | Supersedes the STRICT shadowing sentence of the row above and the "let / var / upd" row: no rebinding within one scope; scope chain prelude -> session -> fn/block; a child may shadow a parent (prelude = hint, user parent = warning); the prelude is read-only. Unquoted `path` and `url` literal types (file paths are no longer strings); `load path`; prelude `default-sound-kit` and late-bound `sound-kit`; `s` takes an optional `kit:` argument, and a non-keyword argument is a sound value used as is. A pattern chain starts with `s`; the first list-valued step gives structure; `midi-notes` is a step after `s`; d1..d9 are the only sinks. Implementation: design-implementation 5.6, 6.5.8, 7.1, 10.1, 11.7. | Decided |
| 2026-09-30 | Formatter and editor syntax | The formatter is a wasm32-clean Rust core (`src/fmt/`) exposed as `vactr fmt`, LSP formatting, a raw wasm export and an editor command. It is gated on a reader pass with no errors. It changes only indentation, trailing blanks and the final newline, and never spacing within a line. Comment lines are re-indented so that `#@` bindings are preserved. Editor highlighting comes from a tree-sitter grammar (a C scanner, built to WASM by CLI 0.27.0 without emscripten, loaded by web-tree-sitter 0.27.0), with the StreamLanguage mode as the fallback. The Rust reader stays the source of truth. See `design-formatter-and-syntax.md`. | Decided |
| 2026-09-30 | Completion engine | One wasm32-clean Rust completion core (`src/complete/`) over the reader's layout skeleton. It is context-aware (call head, `>` pipe target, argument, pair key, `:keyword`, qualified name; nothing in strings or comments), scope-aware (fn, lambda, `for` and block locals, shadowing), ranked (prefix, then subword, then fuzzy; locals, then document, then prelude) and capped at 100. It is used by LSP completion, by a raw wasm export, and by a UI-agnostic editor service with a DOM popup behind a `CompletionSurface` interface, so the canvas editor can implement the same interface. There is no `@codemirror/autocomplete`. The same amendment adds tree-sitter style spans without CodeMirror and a conservative formatter space-indent repair. See `design-completion.md` and `design-formatter-and-syntax.md` 3.9 and 5.5. | Proposed |
| 2026-10-05 | Editor UI style | The editor chrome is solid, flat and square. One token file (`editor/src/app/theme.css`, `--vt-*` custom properties on `:root`, with coarse-pointer overrides) is loaded before `app.css`. Global base rules make every control `appearance: none` with `border-radius: 0`. There is one primary and one danger variant, and separate controls are at least 8px apart. Targets are at least 36px under `pointer: coarse`. The canvas renderer palette is mapped to the tokens as a follow-up for the canvas owner. See `design-ui-style.md`. | Proposed |
| 2026-10-07 | Editor renderer comparison | Canvas is the only code-pane renderer; the DOM comparison backend was removed after measured results showed no performance benefit over canvas. See the archived [comparison report](design-renderer-comparison.md). | Decided |
| 2026-10-08 | Live performance controls | **Momentary tweak.** Right-drag on a bound Direct numeric site (on iPad, a two-finger drag) sets a live value through a separate momentary layer. The document and the tweak slot are never written. On release the value glides back on the audio clock over an editor-local glide time (default 1 s); Shift snaps back. **Stop.** The default stop (`stop-all`, the stop button, `Mod-.`) is gentle: voices release, effect tails ring out until -80 dBFS or a 15 s cap, then the engine is idle. `hush` (`Mod-Shift-.`) cuts with a 5 ms fade and clears the effect state. Both hosts behave the same through the shared `dsp::Engine`. See `design-live-performance.md`. | Accepted |

### Execution Modes

Live coding requires runtime redefinition; Wasm and JIT want a closed program.
Rather than choosing one, vactr defines two modes over the same core language.

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

The formatter (`src/fmt/`) and the editor-only tree-sitter grammar
(`tree-sitter-vact/`, run as WASM through web-tree-sitter) are designed in
`design-formatter-and-syntax.md`. The formatter formats only sources the
reader accepts. It is checked against the reader output
(`sexpr::print_all` equality). The grammar is a lenient superset, used for
highlighting only.

Completion (`src/complete/`, `design-completion.md`) reads the reader's
layout skeleton instead of the node tree. A statement with an error
becomes one childless `Error` node, and text being typed is usually
incomplete, so the node tree would lose the scope around the cursor. The
skeleton keeps it. The LSP, the wasm export and the editor all share the
one engine.

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

The namespace is a reactive dependency graph (author, 2026-09-24): a
change to a value marks everything computed from it dirty -- bindings,
patterns, the slots they are bound to, editor displays -- and only the
affected part recomputes. Recommended structure: demand-driven
incremental computation with static dependency edges from the checker,
revision stamps, and early cutoff (Salsa/Adapton family), with push
notifications to subscribers; see lang-reference.md, section 4.

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

Invariant: a vactr closure is never evaluated on the audio or render
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
| the same core compiled natively inside a Tauri app (shared web UI, native Rust engine on CoreAudio/CoreMIDI); Swift only as glue, AUv3 later | macOS, iPadOS, iOS; lower latency and richer host capabilities; the second deliverable |
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

### Host Tiers (DECIDED, 2026-09-24)

The editor is a Tauri app on macOS and iOS as well as a web app, and the
same Rust core runs in two host tiers behind the capability traits:

| Tier | Engine | Audio | MIDI |
|------|--------|-------|------|
| Browser (first deliverable) | the core compiled to Wasm, in the page | Web Audio, DSP inside the AudioWorklet | Web MIDI |
| Native: macOS and iOS via Tauri (second) | the same core compiled natively, inside the Tauri process (a library inside the app on iOS) | CoreAudio directly (`cpal`), real-time thread, low latency | CoreMIDI |

The WebView only renders the editor and talks to the core over Tauri IPC,
which is the editor-runtime protocol; it never touches audio in the
native tier. Feature sets MAY differ between tiers (author): the native
host advertises richer capabilities -- more voices, long convolution
impulse responses, heavy granular densities, multichannel output,
offline rendering, file access, and later AUv3/plugin hosting -- while
the browser host advertises the subset that fits the worklet budget.
The language is identical on both; using a capability the current host
lacks is a diagnostic ("not available on this host"), never a crash.
Swift is reduced to glue (AVAudioSession, background-audio mode, an
AUv3 extension later) rather than a separate SwiftUI shell. iOS still
forbids JIT, so the native tier runs the bytecode VM.

Canvas-editor amendment (2026-09-30): visible source uses GPU canvas with a DOM
input/accessibility bridge; host tiers above remain authoritative. See
[design-implementation.md section 15.3](design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30)
for editing migration, audible-time telemetry, native/iPad integration, resource
caps and verification gates (workflow session 224; proposed for review).

### Editor Requirements (DECIDED, 2026-09-24)

The dedicated editor (Tauri; also web and Wasm) is part of the product,
not tooling around it. Requirements from the author:

- **Slider panel (right pane).** The editor enumerates every numeric site
  in the current source — literals inside patterns and controls,
  top-level `let`/`var` numbers, `inst` parameter defaults — from the
  checker's AST, and the performer picks any of them to add a slider.
  A slider's value flows back either as a source edit (live
  redefinition of that literal) or as a runtime tweak overlay keyed to
  the site's provenance, without touching the text; both modes exist.
  Nothing about the slider appears in the code (lang-reference.md,
  principle: music code contains only music).
- **Mouse.** Dragging a number in the editor adjusts it the same way.
- **DAW-style parameter editors (author, 2026-09-24).** The slider is
  the generic editor; effect and instrument call sites get an editor
  matched to their meaning, with the operability of a DAW. Each builtin
  declares its parameter groups and their editor kind, so the editor
  panel can show, for a call site chosen from the same enumeration as
  the sliders:
  - EQ (`peq`, `geq`, `dynamic-eq`, `tilt`, `tone`): a frequency/level
    curve with draggable bands (frequency, gain, Q), the live spectrum
    analyzer drawn behind it.
  - Filters (`lpf`, `hpf`, `bpf`, `ladder`, `svf`, `auto-filter`): the
    response curve with a cutoff/resonance handle.
  - Dynamics (`compressor`, `expander`, `gate`, `limiter`, multiband):
    the transfer curve with threshold, ratio, and knee handles, gain
    reduction metered live; multiband editors add crossover handles.
  - Envelopes (`env-adsr`, `env-perc`): the envelope shape with
    draggable stage points.
  - Delay and reverb: taps on a beat-aligned timeline; decay and size
    as a room sketch.
  - Sampler: the waveform with draggable start, end, and loop handles; a
    slice grid (`slice n`) or manual slice markers (`slice [...]`) drawn
    on the waveform, draggable; clicking a slice sets the index pattern
    step; `chop`/`striate` counts as a grid overlay; the bank index `n`
    as a sample browser with the waveform preview (author, 2026-09-24).
  - Wavetable: the frame stack with the position scrubber.
  - Granular: the source waveform with position and spray as a region,
    grain size and density as overlays.
  - LFOs and signals (`sine`, `range`, `lag`): the shape with rate and
    range handles.
  - Pan and spatial: a stereo field; any two parameters: an XY pad.
  - Sequences are written in CODE only (author, 2026-09-24): the step
    grid and the piano roll are DISPLAYS of what is sounding, never
    editors of the list. The numeric parameters inside sequence
    operators do get DAW-style editors: `euclid` a ring with draggable
    hits, steps, and rotation; `maybe` a probability dial; `hold` and
    `fast`/`slow` a length handle on the step display.
  Every handle writes back to the same numeric sites as a slider (source
  edit or tweak overlay), every handle can be MIDI-learned, and the code
  never changes shape because of the editor used.

- **MIDI in.** Controllers map to sliders or directly to sites (MIDI
  learn); in code, `cc n channel:` is a signal usable as any parameter
  and note input is an event source. Web MIDI in the browser, CoreMIDI
  natively.
- **MIDI out.** `midi` is a sink for patterns (design-music.md).
- **MIDI sync.** The cycle clock can follow incoming MIDI clock and
  transport, or send MIDI clock and transport; Ableton Link is the same
  mechanism with a different transport and is planned alongside.
- **Control-panel setup lives in the code, as comments (author,
  2026-09-24).** Everything except the interactive gesture itself must be
  expressible in `.vact`: which parameters of a call site appear in the
  control panel, their editor kind, ranges, and MIDI channel/CC mapping.
  It is written as a **directive comment** on the line it applies to, so
  the music code stays music, indentation is untouched, and the file
  carries the whole setup for a set. Requirements on the notation, to be
  designed separately: one line; a fixed marker after `#` so ordinary
  comments are never mistaken for directives; attaches to the call
  site(s) on its line; names parameters by their keyword; the language
  ignores it entirely (the reader keeps comments with positions for the
  LSP anyway); the editor layer validates it and reports unknown names
  as diagnostics. Length is kept short by four rules (author question
  2026-09-24, "does a filter line get too long?"): ranges and editor
  kinds are never written (they come from the builtin's parameter
  metadata); channel and device defaults are set once at the top of the
  file (`#@ midi ch: 1`); a line names its call sites, with CC numbers
  positional in declared parameter order and `name.param` only to pick
  one; and the editor WRITES learned CC numbers back into the comment,
  so they are rarely typed. A directive on its own line applies to the
  next line when the margin is too short.
  **Shape (author, 2026-09-24, decided):** every directive line starts
  with `#@`; consecutive `#@` lines form one directive block (no closing
  marker, so a forgotten delimiter can never swallow later comments, and
  ordinary `# text` comments stay distinct). The block applies to the
  nearest PRECEDING statement, block, or definition (`fn`, `inst`, `bus`,
  `look`) whose indentation is no deeper
  than the comment's: inside an `inst` body it binds to the previous
  line of the body; at column 0 after the block it binds to the whole
  `inst`. On the same line as code it binds to that line. The
  vocabulary after `#@` is still to be designed; candidate:
  ```vact
  #@ midi ch: 1
  s [:bd :sd] > lpf 800 res: 0.4 > hpf 120 > d1
  #@ lpf cc: 74 71
  #@ hpf cc: 30
  inst analog cutoff: float = 1200 res: float = 0.3:
  	vco :saw freq > ladder cutoff res > * amp
  #@ cutoff res
  ```
  (The alternative `# @ ... @` delimited region was considered and set
  aside: it needs a closing marker and a parser state across lines.)
  **Labels (author, 2026-09-24):** the language has no metadata syntax,
  so a label is also a directive: `#@ name bass-filter` after a block
  labels that block, and any directive anywhere may then address it as
  `bass-filter.lpf` (parameter by keyword). Things that already have a
  name in the code -- `let`, `fn`, `inst`, `bus`, `look`, slots -- are
  labels without a directive. Labels are the preferred identity for
  slider and tweak provenance, since they survive edits that move
  lines; source positions are the fallback for unlabeled sites.
  Placement (author, 2026-09-24): a label goes in the trailing comment
  ON THE LINE IT NAMES (same line = that line); the block-following
  form is for what has no single line, such as an `inst` body.
  Candidate short form, PROPOSED: a leading token ending in `:` is the
  label and the rest of the comment is the directive for that site.
  ```vact
  s [:bd :sd] > lpf 800 res: 0.4 > d1        #@ bass-filter: lpf cc: 74 71
  s [:hh] > hpf 2000 > d2                    #@ hats:
  #@ hats.hpf cc: 30                          # later, by label
  #@ analog.cutoff cc: 1                      # an inst's name is a label
  ```
  Two kinds of directive, told apart by the first token (author,
  2026-09-24): POSITIONAL -- a call-site name or a label definition
  (`#@ lpf hpf`, `#@ bass-filter: ...`, `#@ name X`) -- attaches to the
  same line or the preceding block; ADDRESSED -- a label reference
  (`#@ hats.hpf cc: 30`) or a file-level setting (`#@ midi ch: 1`) -- is
  position-free and may be grouped anywhere, typically as a controller
  map at the end of the file.

- **Visual feedback.** Currently sounding steps and events shown on a
  step grid and a piano roll (display only), slot levels, analyzers
  (design-music.md section 5), and inline diagnostics.

### Packages (DECIDED, 2026-09-24)

External Vactr libraries are imported by repository path, Go style:
`import github.com/owner/name`, versions by git tag with minimal version
selection, `vactr.lock` pinning version and content hash, a cache
under `~/.vactr/pkg`, and a package proxy for the browser build (no
git there). A package is Vactr code plus assets (samples, tables) and
a `vactr.toml` manifest; native extensions are out of v1, so a package
cannot reach the audio thread except through builtins. This supersedes
the earlier "no module system in v1" decision. Builtin catalogs that
packages build on: effects, synthesis models (sampler, analog, FM,
phase distortion, additive, wavetable), and granular (design-music.md
sections 4-6).

### Wasm Constraints

- The runtime crate must build for `wasm32-unknown-unknown` (browser) from the
  start; `wasm32-wasi` is a later addition.
- No assumption of OS threads in the core; concurrency is scheduler-driven.
- All I/O and platform access goes through capability traits.
- Live mode in the browser runs the interpreter/VM compiled to Wasm; Frozen
  mode emits Wasm from vactr programs and is a later milestone.

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
scheduler. `vactr` therefore runs both; an external editor connects to
the same session the console shows.

### Typing

vactr is statically typed with inference (principle 4 in
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

## Static song mode

See [Song mode: reusable finite multi-track parts](design-song-mode.md) for the
proposed finite arrangement, event editing, effect isolation and playback design.
