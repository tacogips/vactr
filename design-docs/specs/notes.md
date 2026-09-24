# Design Notes

This document contains research findings, investigations, and miscellaneous design notes.

## Overview

Notable items that do not fit into architecture or client categories.

---

## Sections

### Why Not Build on Racket / Rhombus (2026-09)

Rhombus was evaluated as a possible base because it already has an
extensible, parenthesis-light syntax (Shrubbery) on top of a Lisp macro
system. Conclusion: borrow ideas, do not adopt the platform.

| Aspect | Rhombus | Relevance to nagamu |
|--------|---------|---------------------|
| Execution path | Rhombus -> Racket -> Racket CS -> Chez Scheme native | Three layers; slow startup, large distribution |
| Performance | Between JavaScript and Ruby on "Are We Fast Yet?" | Acceptable, but not controllable from our side |
| Wasm | No official target; only Chez Scheme has an Emscripten build, which does not cover the Racket runtime | Blocks our browser goal |
| RacketScript | Racket subset to JavaScript, not Wasm; Rhombus depends on the full expander | Not a viable bridge |
| Macros | Very strong | Reference for our expander design |
| Shrubbery notation | Indent/group-based, extensible operators | Reference for the indent rule set; operator precedence is more than we want |

Takeaways adopted into `architecture.md`:

- Own small runtime in Rust, with the S-expression AST fully exposed.
- Keep macros compile-time only; keep `eval` out of the core language
  (Live-mode runtime feature) so a Frozen/Wasm mode stays possible.
- Do not include continuations / `call/cc` in the core.
- Separate GC-managed values from unboxed value types early.

### How Rhombus Writes Conditionals and Maps (2026-09-24)

Checked against the Rhombus repository sources
(`rhombus/rhombus/scribblings/guide/conditional.scrbl` and `guide/map.scrbl`)
because the doc site refuses automated fetches.

| Form | Rhombus | Notes |
|------|---------|-------|
| `if` | `if 1 == 2 \| "same" \| "different"` | Two `\|` alternatives: then, else. No `elif`. |
| `cond` | `cond \| n == 0: 1 \| n == 1: 1 \| ~else: ...` | Each `\|` is test + `:` block; `~else` keyword; error if nothing matches. |
| `match` | `match n \| 0: 1 \| 1: 1 \| _: ...` | Binding patterns; `_` or `~else` as catch-all. |
| multi-clause `fun` | `fun \| fib(0): 1 \| fib(1): 1 \| fib(n): ...` | Fuses definition and match; clauses may differ in arity. |
| `guard` | `guard user_id != "" \| #false` | Early exit from the enclosing block; `guard.let` pattern variant. |
| Map literal | `{"alice": p, "bob": q}` | Braces + `key: value` + commas; shorthand for `Map(["alice", p], ...)`. |
| Empty / typed | `{}`, `Map{...}`, `MutableMap{...}` | Constructor name may prefix the braces. |
| Lookup | `m["alice"]`, `m.maybe["x"]` | Missing key throws; `.maybe` returns `#false`. |
| Extend | `m ++ {"k": v}`, `{& m, "k": v}` | Immutable; `&` splices or binds the rest. |
| Membership | `"alice" in m` | |

Relevance to nagamu:

- Rhombus uses the same `{}` for blocks and maps and disambiguates by
  position. nagamu disambiguates by content (`{key: value}` vs `{expr}`),
  which is simpler for a line-based reader. Recorded in `lang-reference.md`
  section 2 as QA-59.
- `|`-clause syntax for `cond`/`match` is an alternative to `elif` (QA-41)
  worth revisiting once the block rule is settled.
- Rhombus `if` needs no parentheses because it has infix operators and
  `|` delimiters; nagamu gets the same effect from `{}` blocks.

### Deferred: Music-Visual Coupling (2026-09-24)

Music and visuals are specified separately for now (author). The
proposal below was drafted before that decision and is parked here
unchanged. It assumes one machine on the evaluator thread (cycle clock,
slot table, event stream, instrument/look registry, late-bound names)
and couples the two domains through values only.

```nagm
# ---- music into visuals (author intent 2026-09-24; PROPOSED) ----------
# Music reaches visuals only through VALUES, never callbacks. Three
# couplings, plus musical time.
# 1. analysis signals: what it SOUNDS like
fft 0                            # a band level, 0..1
amp                              # overall amplitude
env :d1                          # envelope follower of one slot
# 2. event signals: what it is PLAYING (sample-and-hold per event)
hits :d1                         # 1 at each event of d1, decaying
hits :d1 s: :bd                  # only the kicks: filter by any control
ctrl :d1 :note                   # the last event's note on d1
ctrl :d1 :gain                   # ... or any control
# 3. shared values: one pattern drives both chains; edit it once
let kit [:bd :sd :hh :sd]
kit > s > d1
kit > map look > out o0          # look: keyword -> texture
let pulse [1 0.2 0.6 0.2]
s [:bd :hh :sd :hh] > gain pulse > d1
osc 20 > scale {+ 1 pulse} > out o1     # a number pattern is a signal
                                        # that steps with the cycle
# 4. musical time is a visual signal
osc 20 > rotate {* 6.28 phase} > out o2 # phase: 0..1 within the cycle;
                                        # also `cycle`, `beat`
[{shape 4} {osc 10}] > alt > out o3     # a sequence of textures: visual
                                        # sequencing on the music grid
# signal shaping
lag {hits :d1} 0.1                      # smooth a signal
map-range {ctrl :d1 :note} 48 72 0 1    # rescale, e.g. pitch -> hue
osc 20 > hue {map-range {ctrl :d1 :note} 48 72 0 1} > out
# 5. one machine, two projections: a sound can carry its own LOOK,
#    keyed like an instrument, and a slot then renders both
look :bd-haus:
	shape 4 > scale {+ 1 hit}      # `hit` inside a look: this sound.s own
                                 # event signal
look :sn-dub:
	noise 3 > thresh hit
s [:bd-haus :sn-dub] > d1
looks :d1 > out o0               # the layered looks of what d1 plays
looks :d1 > kaleid 4 > out o1    # an ordinary texture, so it chains
# Everything above reads ONE machine on the evaluator thread: the cycle
# clock, the slot table, the event stream, the instrument/look
# registry, and late-bound names. Music and visuals are two projections
# of it; nothing is duplicated and nothing calls back.
# The reverse direction (input or visual analysis into music: mouse,
# camera brightness) is the same mechanism with signals supplied by the
# graphics capability; later.

```

---
